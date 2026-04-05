use std::path::Path;

use crate::config::AwsDeployConfig;
use crate::error::{SiteError, SiteResult};
use crate::plugin::{Plugin, PluginRegistry};

use super::{count_html_files, require_tool, serialized_cqrs, DeployResult, Deployer};

pub struct AwsDeployer {
    config: AwsDeployConfig,
}

impl AwsDeployer {
    pub(super) fn new(config: AwsDeployConfig) -> Self {
        Self { config }
    }
}

impl Deployer for AwsDeployer {
    fn provider_name(&self) -> &'static str {
        "AWS S3 + CloudFront"
    }

    fn deploy_static(&self, dist_dir: &Path) -> SiteResult<DeployResult> {
        println!("☁️  Deploying to AWS S3: s3://{}", self.config.bucket_name);

        require_tool("aws", "https://aws.amazon.com/cli/")?;

        let sync_output = std::process::Command::new("aws")
            .args([
                "s3",
                "sync",
                dist_dir.to_str().unwrap_or("dist"),
                &format!("s3://{}", self.config.bucket_name),
                "--delete",
                "--region",
                &self.config.region,
                "--exclude",
                "_workers/*",
            ])
            .output()
            .map_err(|e| SiteError::Deploy {
                provider: "aws".into(),
                message: format!("aws s3 sync failed: {}", e),
            })?;

        if !sync_output.status.success() {
            let stderr = String::from_utf8_lossy(&sync_output.stderr);
            return Err(SiteError::Deploy {
                provider: "aws".into(),
                message: stderr.to_string(),
            });
        }

        if let Some(dist_id) = &self.config.cloudfront_distribution_id {
            println!("  Invalidating CloudFront distribution: {}", dist_id);
            let _ = std::process::Command::new("aws")
                .args([
                    "cloudfront",
                    "create-invalidation",
                    "--distribution-id",
                    dist_id,
                    "--paths",
                    "/*",
                ])
                .output();
        }

        let url = self.config.domain.clone().unwrap_or_else(|| {
            format!(
                "https://{}.s3-website-{}.amazonaws.com",
                self.config.bucket_name, self.config.region
            )
        });

        Ok(DeployResult {
            url,
            provider: "AWS S3 + CloudFront".into(),
            pages_deployed: count_html_files(dist_dir),
        })
    }

    fn deploy_workers(&self, _dist_dir: &Path, plugins: &PluginRegistry) -> SiteResult<()> {
        if plugins.is_empty() {
            return Ok(());
        }

        let staging_dir = _dist_dir.join("_ferrosite").join("deploy").join("aws");
        std::fs::create_dir_all(&staging_dir)?;

        println!("⚡ Deploying Lambda functions via AWS SAM/CDK…");
        for plugin in plugins.workers() {
            let plugin_dir = staging_dir.join(&plugin.manifest.name);
            std::fs::create_dir_all(&plugin_dir)?;
            std::fs::write(
                plugin_dir.join("index.js"),
                generate_aws_lambda_worker(plugin),
            )?;
        }
        println!("  Staged Lambda handlers at {}", staging_dir.display());
        println!("  Note: Lambda deployment requires SAM CLI or CDK. Run 'ferrosite deploy-workers --provider aws' for interactive setup.");
        Ok(())
    }
}

pub(crate) fn generate_aws_lambda_worker(plugin: &Plugin) -> String {
    let (commands_json, queries_json) = serialized_cqrs(plugin);
    let manifest = &plugin.manifest;

    format!(
        r#"// Auto-generated CQRS wrapper for plugin: {name}
// Route: {route}
// Worker runtime: aws-lambda

const COMMANDS = {commands};
const QUERIES = {queries};

const CORS_HEADERS = {{
  "Access-Control-Allow-Origin": "*",
  "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
  "Access-Control-Allow-Headers": "Content-Type",
  "Content-Type": "application/json",
}};

function defaultCommandName() {{
    return COMMANDS.length === 1 ? COMMANDS[0].name : null;
}}

function wantsJsonResponse(headers) {{
    return headerValue(headers, "accept").toLowerCase().includes("application/json");
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
    return {{
        statusCode: 303,
        headers: {{
            ...CORS_HEADERS,
            Location: location,
        }},
        body: "",
    }};
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

function headerValue(headers, name) {{
    if (!headers) {{
        return "";
    }}

    const direct = headers[name] ?? headers[name.toLowerCase()] ?? headers[name.toUpperCase()];
    if (typeof direct === "string") {{
        return direct;
    }}

    return "";
}}

function parseCommandRequest(rawBody, contentType) {{
    if (rawBody && typeof rawBody === "object") {{
        return normalizeCommandEnvelope(rawBody);
    }}

    const raw = typeof rawBody === "string" ? rawBody : "";
    const normalizedType = (contentType || "").toLowerCase();

    if (!raw.trim()) {{
        return normalizeCommandEnvelope({{}});
    }}

    if (normalizedType.includes("application/x-www-form-urlencoded") || raw.includes("=")) {{
        return normalizeCommandEnvelope(Object.fromEntries(new URLSearchParams(raw).entries()));
    }}

    return normalizeCommandEnvelope(JSON.parse(raw));
}}

{worker_source}

function jsonResponse(body, statusCode = 200) {{
  return {{
    statusCode,
    headers: CORS_HEADERS,
    body: JSON.stringify(body),
  }};
}}

exports.handler = async function(event, context) {{
  const method = event?.requestContext?.http?.method || event?.httpMethod || "GET";

  if (method === "OPTIONS") {{
    return {{
      statusCode: 204,
      headers: CORS_HEADERS,
      body: "",
    }};
  }}

  try {{
    if (method === "POST") {{
      const rawBody = event?.body || "{{}}";
      const decodedBody = event?.isBase64Encoded
        ? Buffer.from(rawBody, "base64").toString("utf8")
        : rawBody;
            const contentType = headerValue(event?.headers, "content-type");
            const {{ command, payload }} = parseCommandRequest(decodedBody, contentType);

      const known = COMMANDS.find(c => c.name === command);
      if (!known) {{
        return jsonResponse({{ error: `Unknown command: ${{command}}` }}, 400);
      }}

      const result = await handleCommand(command, payload, process.env, context);
            if (!wantsJsonResponse(event?.headers)) {{
                return redirectResponse(redirectLocation(payload, "success"));
            }}
      return jsonResponse({{ ok: true, result }});
    }}

    if (method === "GET") {{
      const params = event?.queryStringParameters || {{}};
      const query = params.query;

      const known = QUERIES.find(q => q.name === query);
      if (!known) {{
        return jsonResponse({{ error: `Unknown query: ${{query}}` }}, 400);
      }}

      const result = await handleQuery(query, params, process.env, context);
      return jsonResponse({{ ok: true, result }});
    }}

    return jsonResponse({{ error: "Method not allowed" }}, 405);
    }} catch (err) {{
        if (method === "POST" && !wantsJsonResponse(event?.headers)) {{
            try {{
                const rawBody = event?.body || "";
                const decodedBody = event?.isBase64Encoded
                    ? Buffer.from(rawBody, "base64").toString("utf8")
                    : rawBody;
                const payload = parseCommandRequest(decodedBody, headerValue(event?.headers, "content-type"));
                return redirectResponse(redirectLocation(payload.payload || payload, "error"));
            }} catch {{
                return redirectResponse(redirectLocation({{}}, "error"));
            }}
        }}
    return jsonResponse({{ error: err.message }}, 500);
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
    fn generates_lambda_wrapper() {
        let output = generate_aws_lambda_worker(&fixture_plugin());
        assert!(output.contains("exports.handler = async function"));
        assert!(output.contains("parseCommandRequest"));
        assert!(output.contains("URLSearchParams"));
        assert!(output.contains("contact-form-success"));
    }
}
