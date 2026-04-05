use std::path::Path;

use crate::config::CloudflareDeployConfig;
use crate::error::{SiteError, SiteResult};
use crate::plugin::{Plugin, PluginRegistry};

use super::{count_html_files, require_tool, serialized_cqrs, CommandExt, DeployResult, Deployer};

pub struct CloudflareDeployer {
    config: CloudflareDeployConfig,
}

impl CloudflareDeployer {
    pub(super) fn new(config: CloudflareDeployConfig) -> Self {
        Self { config }
    }
}

impl Deployer for CloudflareDeployer {
    fn provider_name(&self) -> &'static str {
        "Cloudflare Pages"
    }

    fn deploy_static(&self, dist_dir: &Path) -> SiteResult<DeployResult> {
        println!(
            "🌐 Deploying to Cloudflare Pages: {}",
            self.config.project_name
        );

        require_tool("wrangler", "npm install -g wrangler")?;

        let wrangler_toml = generate_wrangler_toml(&self.config);
        let wrangler_path = dist_dir.parent().unwrap_or(dist_dir).join("wrangler.toml");
        std::fs::write(&wrangler_path, &wrangler_toml)?;

        let mut command = std::process::Command::new("wrangler");
        command.args([
            "pages",
            "deploy",
            dist_dir.to_str().unwrap_or("dist"),
            "--project-name",
            &self.config.project_name,
            "--commit-dirty=true",
        ]);

        let output = command
            .env_or_require("CLOUDFLARE_ACCOUNT_ID", &self.config.account_id)?
            .output()
            .map_err(|e| SiteError::Deploy {
                provider: "cloudflare".into(),
                message: format!("wrangler failed: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SiteError::Deploy {
                provider: "cloudflare".into(),
                message: stderr.to_string(),
            });
        }

        let url = self
            .config
            .domain
            .clone()
            .unwrap_or_else(|| format!("https://{}.pages.dev", self.config.project_name));

        Ok(DeployResult {
            url,
            provider: "Cloudflare Pages".into(),
            pages_deployed: count_html_files(dist_dir),
        })
    }

    fn deploy_workers(&self, dist_dir: &Path, plugins: &PluginRegistry) -> SiteResult<()> {
        if plugins.is_empty() {
            return Ok(());
        }

        println!(
            "⚡ Deploying {} plugin worker(s) to Cloudflare Workers…",
            plugins.len()
        );
        let workers_dir = dist_dir
            .join("_ferrosite")
            .join("deploy")
            .join("cloudflare");
        std::fs::create_dir_all(&workers_dir)?;

        for plugin in plugins.workers() {
            let worker_name = format!("{}-worker", plugin.manifest.name);
            let worker_file = workers_dir.join(format!("{}.worker.js", plugin.manifest.name));
            std::fs::write(&worker_file, generate_cloudflare_worker(plugin))?;

            println!("  Deploying worker: {}", worker_name);

            let output = std::process::Command::new("wrangler")
                .args([
                    "deploy",
                    worker_file.to_str().unwrap_or(""),
                    "--name",
                    &worker_name,
                    "--compatibility-date",
                    "2024-01-01",
                ])
                .output()
                .map_err(|e| SiteError::Deploy {
                    provider: "cloudflare".into(),
                    message: format!("worker deploy failed: {}", e),
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!(
                    "Warning: Worker '{}' deploy failed: {}",
                    worker_name, stderr
                );
            }
        }
        Ok(())
    }
}

pub(crate) fn generate_cloudflare_worker(plugin: &Plugin) -> String {
    let (commands_json, queries_json) = serialized_cqrs(plugin);
    let manifest = &plugin.manifest;

    format!(
        r#"// Auto-generated CQRS wrapper for plugin: {name}
// Route: {route}
// Worker runtime: cloudflare-worker

const COMMANDS = {commands};
const QUERIES = {queries};

const CORS_HEADERS = {{
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type",
}};

{worker_source}

export default {{
  async fetch(request, env, ctx) {{
    if (request.method === "OPTIONS") {{
      return new Response(null, {{ headers: CORS_HEADERS }});
    }}

    const url = new URL(request.url);

    try {{
      if (request.method === "POST") {{
        const body = await request.json();
        const command = body.command;
        const payload = body.payload;

        const known = COMMANDS.find(c => c.name === command);
        if (!known) {{
          return Response.json({{ error: `Unknown command: ${{command}}` }}, {{ status: 400 }});
        }}

        const result = await handleCommand(command, payload, env, ctx);
        return Response.json({{ ok: true, result }}, {{ headers: CORS_HEADERS }});
      }}

      if (request.method === "GET") {{
        const query = url.searchParams.get("query");
        const params = Object.fromEntries(url.searchParams);

        const known = QUERIES.find(q => q.name === query);
        if (!known) {{
          return Response.json({{ error: `Unknown query: ${{query}}` }}, {{ status: 400 }});
        }}

        const result = await handleQuery(query, params, env, ctx);
        return Response.json({{ ok: true, result }}, {{ headers: CORS_HEADERS }});
      }}

      return Response.json({{ error: "Method not allowed" }}, {{ status: 405, headers: CORS_HEADERS }});
    }} catch (err) {{
      return Response.json({{ error: err.message }}, {{ status: 500, headers: CORS_HEADERS }});
    }}
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

fn generate_wrangler_toml(config: &CloudflareDeployConfig) -> String {
    format!(
        r#"name = "{project_name}"
account_id = "{account_id}"
pages_build_output_dir = "dist"
compatibility_date = "2024-01-01"

[vars]
ENVIRONMENT = "production"
"#,
        project_name = config.project_name,
        account_id = config.account_id,
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
    fn generates_cloudflare_wrapper() {
        let output = generate_cloudflare_worker(&fixture_plugin());
        assert!(output.contains("export default"));
    }
}
