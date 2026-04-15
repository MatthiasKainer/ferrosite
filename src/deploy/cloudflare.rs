use std::path::Path;

use crate::config::CloudflareDeployConfig;
use crate::error::{SiteError, SiteResult};
use crate::plugin::{KvNamespace, Plugin, PluginRegistry};

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

        let wrangler_path = dist_dir.parent().unwrap_or(dist_dir).join("wrangler.toml");
        ensure_wrangler_toml(&wrangler_path, &self.config)?;

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
            let worker_config = workers_dir.join(format!("{}.wrangler.toml", plugin.manifest.name));
            std::fs::write(&worker_file, generate_cloudflare_worker(plugin))?;
            std::fs::write(
                &worker_config,
                generate_worker_wrangler_toml(
                    &worker_name,
                    worker_file
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("worker.js"),
                    self.config.domain.as_deref(),
                    &plugin.manifest.worker_route,
                    &plugin.manifest.kv_namespaces,
                ),
            )?;

            println!("  Deploying worker: {}", worker_name);

            let mut command = std::process::Command::new("wrangler");
            command
                .args([
                    "deploy",
                    "--config",
                    worker_config.to_str().unwrap_or("wrangler.toml"),
                ]);

            let output = command
                .env_or_require("CLOUDFLARE_ACCOUNT_ID", &self.config.account_id)?
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

fn ensure_wrangler_toml(path: &Path, config: &CloudflareDeployConfig) -> SiteResult<()> {
    if path.try_exists()? {
        return Ok(());
    }

    std::fs::write(path, generate_wrangler_toml(config))?;
    Ok(())
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

function defaultCommandName() {{
    return COMMANDS.length === 1 ? COMMANDS[0].name : null;
}}

function wantsJsonResponse(request) {{
    const accept = (request.headers.get("accept") || "").toLowerCase();
    return accept.includes("application/json");
}}

function normalizeRedirectTarget(target) {{
    if (typeof target !== "string") {{
        return "/contact/";
    }}

    if (!target.startsWith("/") || target.startsWith("//")) {{
        return "/contact/";
    }}

    return target;
}}

function redirectLocation(payload, status) {{
    const base = normalizeRedirectTarget(payload?.redirect_to);
    const url = new URL(base, "https://ferrosite.local");
    url.hash = status === "success" ? "contact-form-success" : "contact-form-error";
    return `${{url.pathname}}${{url.search}}${{url.hash}}`;
}}

function redirectResponse(location) {{
    return new Response(null, {{
        status: 303,
        headers: {{
            ...CORS_HEADERS,
            Location: location,
        }},
    }});
}}

function normalizeCommandEnvelope(body) {{
    const raw = body && typeof body === "object" && !Array.isArray(body) ? body : {{}};

    if (typeof raw.command === "string" && raw.payload && typeof raw.payload === "object" && !Array.isArray(raw.payload)) {{
        return {{ command: raw.command, payload: raw.payload }};
    }}

    if (typeof raw.command === "string") {{
        const {{ command, ...payload }} = raw;
        return {{ command, payload }};
    }}

    const inferred = defaultCommandName();
    if (inferred) {{
        return {{ command: inferred, payload: raw }};
    }}

    throw new Error("Command is required");
}}

async function parseCommandRequest(request) {{
    const contentType = (request.headers.get("content-type") || "").toLowerCase();

    if (contentType.includes("application/json")) {{
        return normalizeCommandEnvelope(await request.json());
    }}

    if (contentType.includes("application/x-www-form-urlencoded") || contentType.includes("multipart/form-data")) {{
        return normalizeCommandEnvelope(Object.fromEntries((await request.formData()).entries()));
    }}

    const raw = await request.text();
    if (!raw.trim()) {{
        return normalizeCommandEnvelope({{}});
    }}

    try {{
        return normalizeCommandEnvelope(JSON.parse(raw));
    }} catch (err) {{
        if (contentType.includes("text/plain") || raw.includes("=")) {{
            return normalizeCommandEnvelope(Object.fromEntries(new URLSearchParams(raw).entries()));
        }}

        throw err;
    }}
}}

{worker_source}

export default {{
  async fetch(request, env, ctx) {{
    if (request.method === "OPTIONS") {{
      return new Response(null, {{ headers: CORS_HEADERS }});
    }}

    const url = new URL(request.url);
        let submittedPayload = {{}};

    try {{
      if (request.method === "POST") {{
                const {{ command, payload }} = await parseCommandRequest(request);
                submittedPayload = payload;

        const known = COMMANDS.find(c => c.name === command);
        if (!known) {{
          return Response.json({{ error: `Unknown command: ${{command}}` }}, {{ status: 400 }});
        }}

        const result = await handleCommand(command, payload, env, ctx);
                if (!wantsJsonResponse(request)) {{
                    return redirectResponse(redirectLocation(payload, "success"));
                }}
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
            if (request.method === "POST" && !wantsJsonResponse(request)) {{
                return redirectResponse(redirectLocation(submittedPayload, "error"));
            }}
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
pages_build_output_dir = "dist"
compatibility_date = "2024-01-01"

[vars]
ENVIRONMENT = "production"
"#,
        project_name = config.project_name,
    )
}

fn generate_worker_wrangler_toml(
    worker_name: &str,
    worker_entrypoint: &str,
    domain: Option<&str>,
    worker_route: &str,
    kv_namespaces: &[KvNamespace],
) -> String {
    let mut config = format!(
        r#"name = "{worker_name}"
main = "{worker_entrypoint}"
compatibility_date = "2024-01-01"
keep_vars = true
workers_dev = true
"#,
    );

    if let Some((zone_name, route_pattern)) = cloudflare_route(domain, worker_route) {
        config.push_str("\n[route]\n");
        config.push_str(&format!("pattern = \"{route_pattern}\"\n"));
        config.push_str(&format!("zone_name = \"{zone_name}\"\n"));
    }

    for kv in kv_namespaces {
        config.push_str("\n[[kv_namespaces]]\n");
        config.push_str(&format!("binding = \"{}\"\n", kv.binding));
        config.push_str(&format!("id = \"{}\"\n", kv.id));
        if let Some(preview_id) = &kv.preview_id {
            config.push_str(&format!("preview_id = \"{preview_id}\"\n"));
        }
    }

    config
}

fn cloudflare_route(domain: Option<&str>, worker_route: &str) -> Option<(String, String)> {
    let host = domain?
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .trim();

    if host.is_empty() {
        return None;
    }

    let route = worker_route.trim();
    if route.is_empty() || route == "/" {
        return Some((host.to_string(), format!("{host}/*")));
    }

    let normalized_route = if route.starts_with('/') {
        route.to_string()
    } else {
        format!("/{route}")
    };

    Some((host.to_string(), format!("{host}{normalized_route}*")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{PluginManifest, SandboxConfig};
    use std::path::PathBuf;
    use tempfile::tempdir;

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
                kv_namespaces: Vec::new(),
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
        assert!(output.contains("parseCommandRequest"));
        assert!(output.contains("URLSearchParams"));
        assert!(output.contains("contact-form-success"));
    }

    #[test]
    fn pages_wrangler_config_omits_account_id() {
        let output = generate_wrangler_toml(&CloudflareDeployConfig {
            project_name: "matthias-kainer".into(),
            account_id: "abc123".into(),
            workers_subdomain: None,
            domain: None,
        });

        assert!(output.contains("name = \"matthias-kainer\""));
        assert!(output.contains("pages_build_output_dir = \"dist\""));
        assert!(!output.contains("account_id"));
    }

    #[test]
    fn worker_wrangler_config_uses_worker_fields_only() {
        let output = generate_worker_wrangler_toml(
            "contact-form-worker",
            "contact-form.worker.js",
            None,
            "/api/contact",
            &[],
        );

        assert!(output.contains("name = \"contact-form-worker\""));
        assert!(output.contains("main = \"contact-form.worker.js\""));
        assert!(output.contains("keep_vars = true"));
        assert!(output.contains("workers_dev = true"));
        assert!(!output.contains("pages_build_output_dir"));
        assert!(!output.contains("kv_namespaces"));
    }

    #[test]
    fn worker_wrangler_config_adds_route_when_domain_is_configured() {
        let output = generate_worker_wrangler_toml(
            "contact-form-worker",
            "contact-form.worker.js",
            Some("https://matthias-kainer.de/"),
            "/api/contact",
            &[],
        );

        assert!(output.contains("[route]"));
        assert!(output.contains("pattern = \"matthias-kainer.de/api/contact*\""));
        assert!(output.contains("zone_name = \"matthias-kainer.de\""));
    }

    #[test]
    fn worker_wrangler_config_includes_kv_namespaces() {
        use crate::plugin::KvNamespace;
        let kv = vec![KvNamespace {
            binding: "COMMENTS_KV".into(),
            id: "abc123".into(),
            preview_id: Some("preview456".into()),
        }];
        let output = generate_worker_wrangler_toml(
            "draft-comments-worker",
            "draft-comments.worker.js",
            None,
            "/api/draft-comments",
            &kv,
        );

        assert!(output.contains("[[kv_namespaces]]"));
        assert!(output.contains("binding = \"COMMENTS_KV\""));
        assert!(output.contains("id = \"abc123\""));
        assert!(output.contains("preview_id = \"preview456\""));
    }

    #[test]
    fn existing_pages_wrangler_config_is_not_overwritten() {
        let temp = tempdir().expect("tempdir");
        let wrangler_path = temp.path().join("wrangler.toml");
        let original = "name = \"custom\"\ncompatibility_date = \"2025-01-01\"\n";
        std::fs::write(&wrangler_path, original).expect("write existing wrangler config");

        ensure_wrangler_toml(
            &wrangler_path,
            &CloudflareDeployConfig {
                project_name: "matthias-kainer".into(),
                account_id: "abc123".into(),
                workers_subdomain: None,
                domain: None,
            },
        )
        .expect("preserve existing wrangler config");

        let preserved = std::fs::read_to_string(&wrangler_path).expect("read wrangler config");
        assert_eq!(preserved, original);
    }
}
