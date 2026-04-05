use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Declarative plugin manifest — defines what the plugin provides and requires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub author: String,
    #[serde(default)]
    pub slots: Vec<String>,
    #[serde(default)]
    pub head_inject: Vec<HeadInjection>,
    #[serde(default)]
    pub commands: Vec<CommandDef>,
    #[serde(default)]
    pub queries: Vec<QueryDef>,
    pub component_file: String,
    pub worker_file: String,
    pub worker_route: String,
    #[serde(default = "default_worker_runtime")]
    pub worker_runtime: String,
    #[serde(default)]
    pub required_env: Vec<String>,
    #[serde(default)]
    pub sandbox: SandboxConfig,
}

#[derive(Debug, Deserialize)]
struct PluginManifestDocument {
    #[serde(default)]
    plugin: Option<PluginManifestCore>,
    #[serde(default)]
    head_inject: Vec<HeadInjection>,
    #[serde(default)]
    commands: Vec<CommandDef>,
    #[serde(default)]
    queries: Vec<QueryDef>,
    #[serde(default)]
    sandbox: SandboxConfig,
}

#[derive(Debug, Clone, Deserialize)]
struct PluginManifestCore {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub author: String,
    pub slots: Vec<String>,
    #[serde(default)]
    pub head_inject: Vec<HeadInjection>,
    #[serde(default)]
    pub commands: Vec<CommandDef>,
    #[serde(default)]
    pub queries: Vec<QueryDef>,
    pub component_file: String,
    pub worker_file: String,
    pub worker_route: String,
    #[serde(default = "default_worker_runtime")]
    pub worker_runtime: String,
    #[serde(default)]
    pub required_env: Vec<String>,
    #[serde(default)]
    pub sandbox: SandboxConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SandboxConfig {
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u32,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u32,
    #[serde(default)]
    pub allowed_domains: Vec<String>,
    #[serde(default = "default_true")]
    pub cors: bool,
    #[serde(default = "default_rate_limit")]
    pub rate_limit_rpm: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadInjection {
    pub kind: HeadInjectionKind,
    pub value: String,
    #[serde(default)]
    pub attrs: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HeadInjectionKind {
    Script,
    Style,
    Link,
    Meta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDef {
    pub name: String,
    pub description: String,
    pub payload_schema: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryDef {
    pub name: String,
    pub description: String,
    pub params_schema: serde_json::Value,
    pub response_schema: serde_json::Value,
}

pub(super) fn parse_manifest(raw: &str) -> Result<PluginManifest, toml::de::Error> {
    match toml::from_str::<PluginManifest>(raw) {
        Ok(manifest) => Ok(manifest),
        Err(flat_error) => {
            let doc: PluginManifestDocument = toml::from_str(raw)?;
            if let Some(plugin) = doc.plugin {
                Ok(PluginManifest {
                    name: plugin.name,
                    version: plugin.version,
                    description: plugin.description,
                    author: plugin.author,
                    slots: plugin.slots,
                    head_inject: if doc.head_inject.is_empty() {
                        plugin.head_inject
                    } else {
                        doc.head_inject
                    },
                    commands: if doc.commands.is_empty() {
                        plugin.commands
                    } else {
                        doc.commands
                    },
                    queries: if doc.queries.is_empty() {
                        plugin.queries
                    } else {
                        doc.queries
                    },
                    component_file: plugin.component_file,
                    worker_file: plugin.worker_file,
                    worker_route: plugin.worker_route,
                    worker_runtime: plugin.worker_runtime,
                    required_env: plugin.required_env,
                    sandbox: if doc.sandbox == SandboxConfig::default() {
                        plugin.sandbox
                    } else {
                        doc.sandbox
                    },
                })
            } else {
                Err(flat_error)
            }
        }
    }
}

fn default_worker_runtime() -> String {
    "cloudflare-worker".into()
}

fn default_memory_mb() -> u32 {
    128
}

fn default_timeout_ms() -> u32 {
    5000
}

fn default_true() -> bool {
    true
}

fn default_rate_limit() -> u32 {
    60
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_plugin_manifest() {
        let raw = r#"
[plugin]
name = "contact-form"
version = "1.0.0"
description = "Contact form"
author = "ferrosite"
slots = ["contact-form"]
component_file = "component.js"
worker_file = "worker.js"
worker_route = "/api/contact"
worker_runtime = "cloudflare-worker"
required_env = ["RESEND_API_KEY", "TO_EMAIL"]

[[commands]]
name = "SendMessage"
description = "Send a message"
payload_schema = { type = "object" }

[sandbox]
memory_mb = 128
timeout_ms = 5000
allowed_domains = ["api.resend.com"]
cors = true
rate_limit_rpm = 10
"#;

        let manifest = parse_manifest(raw).unwrap();
        assert_eq!(manifest.name, "contact-form");
        assert_eq!(manifest.worker_route, "/api/contact");
        assert_eq!(manifest.commands.len(), 1);
        assert_eq!(manifest.required_env, vec!["RESEND_API_KEY", "TO_EMAIL"]);
    }

    #[test]
    fn parses_flat_manifest_with_defaults() {
        let raw = r#"
name = "contact-form"
version = "1.0.0"
author = "ferrosite"
slots = ["contact-form"]
component_file = "component.js"
worker_file = "worker.js"
worker_route = "/api/contact"
"#;

        let manifest = parse_manifest(raw).expect("flat manifest should parse");

        assert_eq!(manifest.worker_runtime, "cloudflare-worker");
        assert_eq!(manifest.sandbox, SandboxConfig::default());
        assert!(manifest.head_inject.is_empty());
    }

    #[test]
    fn top_level_sections_override_nested_plugin_sections() {
        let raw = r#"
[plugin]
name = "search"
version = "1.0.0"
author = "ferrosite"
slots = ["search"]
component_file = "component.js"
worker_file = "worker.js"
worker_route = "/api/search"

[[plugin.commands]]
name = "LegacyCommand"
description = "Legacy"
payload_schema = { type = "object" }

[[commands]]
name = "Search"
description = "Current"
payload_schema = { type = "object" }

[plugin.sandbox]
memory_mb = 128
timeout_ms = 5000
cors = true
rate_limit_rpm = 10

[sandbox]
memory_mb = 256
timeout_ms = 8000
cors = false
rate_limit_rpm = 30
"#;

        let manifest = parse_manifest(raw).expect("nested manifest should parse");

        assert_eq!(manifest.commands.len(), 1);
        assert_eq!(manifest.commands[0].name, "Search");
        assert_eq!(manifest.sandbox.memory_mb, 256);
        assert_eq!(manifest.sandbox.timeout_ms, 8000);
        assert!(!manifest.sandbox.cors);
    }
}
