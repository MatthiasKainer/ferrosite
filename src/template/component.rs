use std::path::Path;
use walkdir::WalkDir;

use crate::error::{io_with_path, SiteResult};

// ── pfusch Component Registry ──────────────────────────────────────────────────

/// A loaded set of pfusch component scripts, ready to be injected into pages.
#[derive(Debug, Clone, Default)]
pub struct ComponentRegistry {
    /// Map of component-name → JS source
    components: Vec<ComponentDef>,
    /// External CDN URL for pfusch.js itself
    pub pfusch_cdn: String,
}

#[derive(Debug, Clone)]
pub struct ComponentDef {
    pub name: String,
    pub source: String,
    /// Whether this component requires SSR (Puppeteer pass)
    pub requires_ssr: bool,
    /// Custom element tag names defined by this source file
    pub tag_names: Vec<String>,
}

impl ComponentRegistry {
    pub fn new(pfusch_cdn: &str) -> Self {
        Self {
            components: Vec::new(),
            pfusch_cdn: pfusch_cdn.to_string(),
        }
    }

    /// Load all `.js` component files from a directory — side effect: reads files.
    pub fn load_from_dir(dir: &Path, pfusch_cdn: &str) -> SiteResult<Self> {
        let mut registry = Self::new(pfusch_cdn);

        if !dir.exists() {
            return Ok(registry);
        }

        for entry in WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "js"))
        {
            let path = entry.path();
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let source = std::fs::read_to_string(path)
                .map_err(io_with_path(path, "reading component file"))?;
            let requires_ssr = source.contains("// @ssr");
            let tag_names = parse_component_tag_names(&source);

            registry.components.push(ComponentDef {
                name,
                source,
                requires_ssr,
                tag_names,
            });
        }

        Ok(registry)
    }

    /// Merge plugin components into this registry — pure accumulation.
    pub fn with_plugin_components(mut self, plugin_components: Vec<ComponentDef>) -> Self {
        self.components.extend(plugin_components);
        self
    }

    /// Generate the `<script type="module">` block for all components — pure function.
    pub fn render_script_block(&self) -> String {
        if self.components.is_empty() {
            return format!(
                r#"<script type="module" src="{}"></script>"#,
                self.pfusch_cdn
            );
        }

        let component_sources: String = self
            .components
            .iter()
            .map(|c| format!("// === {} ===\n{}\n", c.name, c.source))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"<script type="module">
import {{ pfusch, html, css }} from "{}";

{}
</script>"#,
            self.pfusch_cdn, component_sources
        )
    }

    /// Whether any component requires the Puppeteer SSR pass.
    pub fn any_requires_ssr(&self) -> bool {
        self.components.iter().any(|c| c.requires_ssr)
    }

    /// List of component names requiring SSR.
    pub fn ssr_components(&self) -> Vec<&str> {
        self.components
            .iter()
            .filter(|c| c.requires_ssr)
            .map(|c| c.name.as_str())
            .collect()
    }

    /// Custom element tags that should trigger an SSR pass when present on a page.
    pub fn ssr_component_tags(&self) -> Vec<&str> {
        self.components
            .iter()
            .filter(|c| c.requires_ssr)
            .flat_map(|c| c.tag_names.iter().map(String::as_str))
            .collect()
    }
}

pub(crate) fn parse_component_tag_names(source: &str) -> Vec<String> {
    let mut tags = Vec::new();
    collect_tag_names(source, r#"pfusch(""#, &mut tags);
    collect_tag_names(source, r#"pfusch('"#, &mut tags);
    collect_tag_names(source, r#"customElements.define(""#, &mut tags);
    collect_tag_names(source, r#"customElements.define('"#, &mut tags);
    tags
}

fn collect_tag_names(source: &str, marker: &str, tags: &mut Vec<String>) {
    let quote = marker.chars().last().unwrap_or('"');
    let mut search_start = 0;

    while let Some(offset) = source[search_start..].find(marker) {
        let start = search_start + offset + marker.len();
        let rest = &source[start..];
        let Some(end) = rest.find(quote) else {
            break;
        };

        let tag = &rest[..end];
        if tag.contains('-') && !tags.iter().any(|existing| existing == tag) {
            tags.push(tag.to_string());
        }

        search_start = start + end + 1;
    }
}

// ── Component HTML helpers ─────────────────────────────────────────────────────

/// Render a pfusch custom element tag with JSON attributes — pure function.
///
/// Generates: `<tag-name attr1="val1" data-json='...'></tag-name>`
pub fn render_component_tag(
    tag_name: &str,
    attrs: &[(&str, &str)],
    json_data: Option<&serde_json::Value>,
    slot_attr: Option<&str>,
) -> String {
    let mut html = format!("<{}", tag_name);

    for (k, v) in attrs {
        html.push_str(&format!(r#" {}="{}""#, k, escape_attr(v)));
    }

    if let Some(data) = json_data {
        let json_str = serde_json::to_string(data).unwrap_or_default();
        html.push_str(&format!(
            r#" data-json='{}'"#,
            json_str.replace('\'', "&apos;")
        ));
    }

    if let Some(slot) = slot_attr {
        html.push_str(&format!(r#" slot="{}""#, slot));
    }

    html.push('>');
    html.push_str(&format!("</{}>", tag_name));
    html
}

/// Escape a value for use in an HTML attribute — pure function.
fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Generate the pfusch `<style id="pfusch-style">` block from CSS vars — pure function.
pub fn render_pfusch_style(css_vars: &str) -> String {
    format!(
        r#"<style id="pfusch-style">
{}
</style>"#,
        css_vars
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_component_tag_escapes_attributes_and_embeds_json_payload() {
        let tag = render_component_tag(
            "contact-form",
            &[("title", "A \"quoted\" <title>"), ("data-id", "abc&123")],
            Some(&serde_json::json!({ "message": "it's live" })),
            Some("hero"),
        );

        assert!(tag.starts_with("<contact-form"));
        assert!(tag.contains(r#"title="A &quot;quoted&quot; &lt;title&gt;""#));
        assert!(tag.contains(r#"data-id="abc&amp;123""#));
        assert!(tag.contains(r#"data-json='{"message":"it&apos;s live"}'"#));
        assert!(tag.contains(r#"slot="hero""#));
        assert!(tag.ends_with("</contact-form>"));
    }

    #[test]
    fn render_script_block_collects_component_sources_and_tracks_ssr() {
        let registry = ComponentRegistry::new("https://cdn.example/pfusch.js")
            .with_plugin_components(vec![
                ComponentDef {
                    name: "hero-card".into(),
                    source: "customElements.define('hero-card', class {});".into(),
                    requires_ssr: false,
                    tag_names: vec!["hero-card".into()],
                },
                ComponentDef {
                    name: "contact-form".into(),
                    source: "// @ssr\ncustomElements.define('contact-form', class {});".into(),
                    requires_ssr: true,
                    tag_names: vec!["contact-form".into()],
                },
            ]);

        let script = registry.render_script_block();

        assert!(script.contains("import { pfusch, html, css }"));
        assert!(script.contains("// === hero-card ==="));
        assert!(script.contains("// === contact-form ==="));
        assert!(registry.any_requires_ssr());
        assert_eq!(registry.ssr_components(), vec!["contact-form"]);
        assert_eq!(registry.ssr_component_tags(), vec!["contact-form"]);
    }

    #[test]
    fn parse_component_tag_names_collects_pfusch_and_custom_element_tags() {
        let source = r#"
pfusch("dev-project-card", {});
customElements.define('ferrosite-contact-form', class {});
pfusch("dev-project-grid", {});
"#;

        assert_eq!(
            parse_component_tag_names(source),
            vec![
                "dev-project-card".to_string(),
                "dev-project-grid".to_string(),
                "ferrosite-contact-form".to_string(),
            ]
        );
    }

    #[test]
    fn render_pfusch_style_wraps_css_variables() {
        let style = render_pfusch_style(":root { --color-primary: red; }");

        assert!(style.contains(r#"<style id="pfusch-style">"#));
        assert!(style.contains("--color-primary: red;"));
        assert!(style.contains("</style>"));
    }
}
