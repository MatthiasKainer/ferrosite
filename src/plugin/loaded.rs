use std::path::{Path, PathBuf};

use crate::error::{io_with_path, SiteError, SiteResult};
use crate::template::component::{parse_component_tag_names, ComponentDef};

use super::manifest::parse_manifest;
use super::{HeadInjectionKind, PluginManifest};

/// A loaded plugin with its manifest and source files.
#[derive(Debug, Clone)]
pub struct Plugin {
    pub manifest: PluginManifest,
    pub component_source: String,
    pub worker_source: String,
    pub dir: PathBuf,
}

impl Plugin {
    pub fn from_dir(plugin_dir: &Path) -> SiteResult<Self> {
        let manifest_path = plugin_dir.join("manifest.toml");
        if !manifest_path.exists() {
            return Err(SiteError::Plugin {
                plugin: plugin_dir.display().to_string(),
                message: "manifest.toml not found".into(),
            });
        }

        let manifest_raw = std::fs::read_to_string(&manifest_path)
            .map_err(io_with_path(&manifest_path, "reading plugin manifest"))?;
        let manifest = parse_manifest(&manifest_raw).map_err(|e| SiteError::Plugin {
            plugin: plugin_dir.display().to_string(),
            message: format!("manifest parse error: {}", e),
        })?;

        let component_path = plugin_dir.join(&manifest.component_file);
        let component_source =
            std::fs::read_to_string(&component_path).map_err(|e| SiteError::Plugin {
                plugin: manifest.name.clone(),
                message: io_with_path(&component_path, "reading plugin component")(e).to_string(),
            })?;

        let worker_path = plugin_dir.join(&manifest.worker_file);
        let worker_source =
            std::fs::read_to_string(&worker_path).map_err(|e| SiteError::Plugin {
                plugin: manifest.name.clone(),
                message: io_with_path(&worker_path, "reading plugin worker")(e).to_string(),
            })?;

        Ok(Self {
            manifest,
            component_source,
            worker_source,
            dir: plugin_dir.to_path_buf(),
        })
    }

    pub fn to_component_def(&self) -> ComponentDef {
        ComponentDef {
            name: self.manifest.name.clone(),
            source: self.component_source.clone(),
            requires_ssr: self.component_source.contains("// @ssr"),
            tag_names: parse_component_tag_names(&self.component_source),
        }
    }

    pub fn render_head_injections(&self) -> String {
        self.manifest
            .head_inject
            .iter()
            .map(|h| match h.kind {
                HeadInjectionKind::Script => {
                    let attrs: String = h
                        .attrs
                        .iter()
                        .map(|(k, v)| format!(r#" {}="{}""#, k, v))
                        .collect();
                    format!(r#"<script src="{}"{}></script>"#, h.value, attrs)
                }
                HeadInjectionKind::Style => {
                    format!(r#"<link rel="stylesheet" href="{}"/>"#, h.value)
                }
                HeadInjectionKind::Link => {
                    let attrs: String = h
                        .attrs
                        .iter()
                        .map(|(k, v)| format!(r#" {}="{}""#, k, v))
                        .collect();
                    format!(r#"<link href="{}"{}>"#, h.value, attrs)
                }
                HeadInjectionKind::Meta => {
                    let attrs: String = h
                        .attrs
                        .iter()
                        .map(|(k, v)| format!(r#" {}="{}""#, k, v))
                        .collect();
                    format!(r#"<meta content="{}"{}>"#, h.value, attrs)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn api_endpoint(&self, base_url: &str) -> String {
        let base = base_url.trim_end_matches('/');
        let route = self.manifest.worker_route.trim_start_matches('/');
        format!("{}/{}", base, route)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{HeadInjection, SandboxConfig};
    use std::collections::HashMap;

    fn plugin_with_head_injections(head_inject: Vec<HeadInjection>) -> Plugin {
        Plugin {
            manifest: PluginManifest {
                name: "contact-form".into(),
                version: "1.0.0".into(),
                description: String::new(),
                author: "ferrosite".into(),
                slots: vec!["contact-form".into()],
                head_inject,
                commands: Vec::new(),
                queries: Vec::new(),
                component_file: "component.js".into(),
                worker_file: "worker.js".into(),
                worker_route: "/api/contact".into(),
                worker_runtime: "cloudflare-worker".into(),
                required_env: Vec::new(),
                sandbox: SandboxConfig::default(),
            },
            component_source: "customElements.define('contact-form', class {});".into(),
            worker_source: "export default {}".into(),
            dir: PathBuf::from("/plugins/contact-form"),
        }
    }

    #[test]
    fn api_endpoint_normalizes_slashes() {
        let plugin = plugin_with_head_injections(Vec::new());

        assert_eq!(
            plugin.api_endpoint("https://example.com/"),
            "https://example.com/api/contact"
        );
    }

    #[test]
    fn render_head_injections_outputs_html_tags_for_each_injection_type() {
        let plugin = plugin_with_head_injections(vec![
            HeadInjection {
                kind: HeadInjectionKind::Script,
                value: "https://cdn.example/app.js".into(),
                attrs: HashMap::from([("defer".into(), "true".into())]),
            },
            HeadInjection {
                kind: HeadInjectionKind::Style,
                value: "https://cdn.example/app.css".into(),
                attrs: HashMap::new(),
            },
            HeadInjection {
                kind: HeadInjectionKind::Link,
                value: "https://example.com/preload".into(),
                attrs: HashMap::from([("rel".into(), "preload".into())]),
            },
            HeadInjection {
                kind: HeadInjectionKind::Meta,
                value: "summary".into(),
                attrs: HashMap::from([("name".into(), "twitter:card".into())]),
            },
        ]);

        let html = plugin.render_head_injections();

        assert!(html.contains(r#"<script src="https://cdn.example/app.js""#));
        assert!(html.contains(r#"defer="true""#));
        assert!(html.contains(r#"<link rel="stylesheet" href="https://cdn.example/app.css"/>"#));
        assert!(html.contains(r#"<link href="https://example.com/preload""#));
        assert!(html.contains(r#"rel="preload""#));
        assert!(html.contains(r#"<meta content="summary""#));
        assert!(html.contains(r#"name="twitter:card""#));
    }

    #[test]
    fn to_component_def_reuses_plugin_name_and_source() {
        let plugin = plugin_with_head_injections(Vec::new());

        let component = plugin.to_component_def();

        assert_eq!(component.name, "contact-form");
        assert_eq!(
            component.source,
            "customElements.define('contact-form', class {});"
        );
        assert!(!component.requires_ssr);
    }
}
