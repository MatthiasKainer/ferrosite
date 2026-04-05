use std::path::Path;

use crate::config::{load_site_config_for_root, DeployProvider, SiteConfig};
use crate::error::{SiteError, SiteResult};
use crate::plugin::{Plugin, PluginRegistry};

mod aws;
mod azure;
mod cloudflare;

use aws::AwsDeployer;
use azure::AzureDeployer;
use cloudflare::CloudflareDeployer;

pub(crate) use cloudflare::generate_cloudflare_worker;

pub trait Deployer {
    fn deploy_static(&self, dist_dir: &Path) -> SiteResult<DeployResult>;
    fn deploy_workers(&self, dist_dir: &Path, plugins: &PluginRegistry) -> SiteResult<()>;
    fn provider_name(&self) -> &'static str;
}

#[derive(Debug)]
pub struct DeployResult {
    pub url: String,
    pub provider: String,
    pub pages_deployed: usize,
}

/// Create the appropriate deployer for the configured provider.
pub fn make_deployer(config: &SiteConfig) -> SiteResult<Box<dyn Deployer>> {
    match config.deploy.provider {
        DeployProvider::Cloudflare => {
            let cf_config =
                config.deploy.cloudflare.as_ref().ok_or_else(|| {
                    SiteError::Config("Missing [deploy.cloudflare] section".into())
                })?;
            Ok(Box::new(CloudflareDeployer::new(cf_config.clone())))
        }
        DeployProvider::Aws => {
            let aws_config = config
                .deploy
                .aws
                .as_ref()
                .ok_or_else(|| SiteError::Config("Missing [deploy.aws] section".into()))?;
            Ok(Box::new(AwsDeployer::new(aws_config.clone())))
        }
        DeployProvider::Azure => {
            let azure_config = config
                .deploy
                .azure
                .as_ref()
                .ok_or_else(|| SiteError::Config("Missing [deploy.azure] section".into()))?;
            Ok(Box::new(AzureDeployer::new(azure_config.clone())))
        }
    }
}

pub fn deploy_site(site_root: &Path) -> SiteResult<DeployResult> {
    let config = load_site_config_for_root(site_root)?;
    let dist_dir = site_root.join(&config.build.output_dir);

    if !dist_dir.exists() {
        return Err(SiteError::Deploy {
            provider: config.deploy.provider.to_str().to_string(),
            message: format!(
                "Output directory '{}' not found. Run 'ferrosite build' first.",
                dist_dir.display()
            ),
        });
    }

    let deployer = make_deployer(&config)?;
    println!("🚀 Deploying with {}…", deployer.provider_name());

    let plugins_dir = site_root.join("plugins");
    let plugins = PluginRegistry::load_from_dir(&plugins_dir, &config.plugins.enabled)?;

    let result = deployer.deploy_static(&dist_dir)?;
    deployer.deploy_workers(&dist_dir, &plugins)?;

    println!("🎉 Deployed! Live at: {}", result.url);
    Ok(result)
}

pub(super) fn require_tool(name: &str, install_hint: &str) -> SiteResult<()> {
    let found = std::process::Command::new(name)
        .arg("--version")
        .output()
        .is_ok();

    if !found {
        return Err(SiteError::Deploy {
            provider: name.to_string(),
            message: format!("'{}' not found in PATH. Install it: {}", name, install_hint),
        });
    }
    Ok(())
}

pub(super) fn count_html_files(dir: &Path) -> usize {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "html"))
        .count()
}

pub(super) fn serialized_cqrs(plugin: &Plugin) -> (String, String) {
    let commands = serde_json::to_string_pretty(&plugin.manifest.commands).unwrap_or_default();
    let queries = serde_json::to_string_pretty(&plugin.manifest.queries).unwrap_or_default();
    (commands, queries)
}

pub(super) trait CommandExt {
    fn env_or_require(self, var: &str, fallback: &str) -> SiteResult<std::process::Command>;
}

impl CommandExt for std::process::Command {
    fn env_or_require(mut self, var: &str, fallback: &str) -> SiteResult<std::process::Command> {
        let value = std::env::var(var).unwrap_or_else(|_| fallback.to_string());
        self.env(var, value);
        Ok(self)
    }
}

impl DeployProvider {
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::Cloudflare => "cloudflare",
            Self::Aws => "aws",
            Self::Azure => "azure",
        }
    }
}
