use std::path::Path;

use crate::config::AzureDeployConfig;
use crate::error::{SiteError, SiteResult};
use crate::plugin::{Plugin, PluginRegistry};

use super::{count_html_files, require_tool, serialized_cqrs, DeployResult, Deployer};

pub struct AzureDeployer {
    config: AzureDeployConfig,
}

impl AzureDeployer {
    pub(super) fn new(config: AzureDeployConfig) -> Self {
        Self { config }
    }
}

impl Deployer for AzureDeployer {
    fn provider_name(&self) -> &'static str {
        "Azure Static Web Apps"
    }

    fn deploy_static(&self, dist_dir: &Path) -> SiteResult<DeployResult> {
        println!(
            "🔷 Deploying to Azure Static Web Apps: {}",
            self.config.app_name
        );

        require_tool(
            "az",
            "https://docs.microsoft.com/cli/azure/install-azure-cli",
        )?;
        require_tool("swa", "npm install -g @azure/static-web-apps-cli")?;

        let swa_config = generate_swa_config();
        std::fs::write(dist_dir.join("staticwebapp.config.json"), swa_config)?;

        let output = std::process::Command::new("swa")
            .args([
                "deploy",
                dist_dir.to_str().unwrap_or("dist"),
                "--app-name",
                &self.config.app_name,
                "--resource-group",
                &self.config.resource_group,
                "--env",
                "production",
            ])
            .output()
            .map_err(|e| SiteError::Deploy {
                provider: "azure".into(),
                message: format!("swa deploy failed: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SiteError::Deploy {
                provider: "azure".into(),
                message: stderr.to_string(),
            });
        }

        Ok(DeployResult {
            url: format!("https://{}.azurestaticapps.net", self.config.app_name),
            provider: "Azure Static Web Apps".into(),
            pages_deployed: count_html_files(dist_dir),
        })
    }

    fn deploy_workers(&self, _dist_dir: &Path, plugins: &PluginRegistry) -> SiteResult<()> {
        if plugins.is_empty() {
            return Ok(());
        }

        let staging_dir = _dist_dir.join("_ferrosite").join("deploy").join("azure");
        std::fs::create_dir_all(&staging_dir)?;

        for plugin in plugins.workers() {
            let plugin_dir = staging_dir.join(&plugin.manifest.name);
            std::fs::create_dir_all(&plugin_dir)?;
            std::fs::write(
                plugin_dir.join("index.js"),
                generate_azure_function_worker(plugin),
            )?;
        }

        println!("⚡ Azure Functions for plugins — use 'func deploy' or Azure DevOps pipeline.");
        println!(
            "  Staged Azure Function handlers at {}",
            staging_dir.display()
        );
        Ok(())
    }
}

pub(crate) fn generate_azure_function_worker(plugin: &Plugin) -> String {
    let (commands_json, queries_json) = serialized_cqrs(plugin);
    let manifest = &plugin.manifest;

    format!(
        r#"// Auto-generated CQRS wrapper for plugin: {name}
// Route: {route}
// Worker runtime: azure-function

const COMMANDS = {commands};
const QUERIES = {queries};

const CORS_HEADERS = {{
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type",
  "Content-Type": "application/json",
}};

{worker_source}

function response(body, status = 200) {{
  return {{
    status,
    headers: CORS_HEADERS,
    body: JSON.stringify(body),
  }};
}}

module.exports = async function(context, req) {{
  const method = req?.method || "GET";

  if (method === "OPTIONS") {{
    context.res = {{
      status: 204,
      headers: CORS_HEADERS,
      body: "",
    }};
    return;
  }}

  try {{
    if (method === "POST") {{
      const body = req?.body || {{}};
      const command = body.command;
      const payload = body.payload;

      const known = COMMANDS.find(c => c.name === command);
      if (!known) {{
        context.res = response({{ error: `Unknown command: ${{command}}` }}, 400);
        return;
      }}

      const result = await handleCommand(command, payload, process.env, context);
      context.res = response({{ ok: true, result }});
      return;
    }}

    if (method === "GET") {{
      const params = req?.query || {{}};
      const query = params.query;

      const known = QUERIES.find(q => q.name === query);
      if (!known) {{
        context.res = response({{ error: `Unknown query: ${{query}}` }}, 400);
        return;
      }}

      const result = await handleQuery(query, params, process.env, context);
      context.res = response({{ ok: true, result }});
      return;
    }}

    context.res = response({{ error: "Method not allowed" }}, 405);
  }} catch (err) {{
    context.res = response({{ error: err.message }}, 500);
  }}
}};
"#,
        name = manifest.name,
        route = manifest.worker_route,
        commands = commands_json,
        queries = queries_json,
        worker_source = plugin.worker_source,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{PluginManifest, SandboxConfig};
    use std::path::PathBuf;

    fn fixture_plugin() -> Plugin {
        Plugin {
            manifest: PluginManifest {
                name: "contact-form".into(),
                version: "1.0.0".into(),
                description: String::new(),
                author: "ferrosite".into(),
                slots: vec!["contact-form".into()],
                head_inject: Vec::new(),
                commands: Vec::new(),
                queries: Vec::new(),
                component_file: "component.js".into(),
                worker_file: "worker.js".into(),
                worker_route: "/api/contact".into(),
                worker_runtime: "cloudflare-worker".into(),
                required_env: Vec::new(),
                sandbox: SandboxConfig::default(),
            },
            component_source: String::new(),
            worker_source:
                "async function handleCommand() { return { ok: true }; }\nasync function handleQuery() { return { ok: true }; }"
                    .into(),
            dir: PathBuf::from("plugins/contact-form"),
        }
    }

    #[test]
    fn generates_azure_wrapper() {
        let output = generate_azure_function_worker(&fixture_plugin());
        assert!(output.contains("module.exports = async function"));
    }
}

fn generate_swa_config() -> String {
    r#"{
  "navigationFallback": {
    "rewrite": "/index.html",
    "exclude": ["/assets/*", "*.css", "*.js", "*.png", "*.jpg", "*.ico"]
  },
  "responseOverrides": {
    "404": { "rewrite": "/404/index.html", "statusCode": 404 }
  },
  "mimeTypes": {
    ".mjs": "application/javascript"
  }
}"#
    .into()
}
