use serde::{Deserialize, Serialize};

use crate::content::slot::{SlotAssignment, SlotType};
use crate::error::{SiteError, SiteResult};

// ── Frontmatter ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    pub slot: String,
    #[serde(default)]
    pub order: i32,
    #[serde(default = "default_weight")]
    pub weight: i32,
    #[serde(default = "default_star")]
    pub page_scope: String,
    #[serde(default)]
    pub draft: bool,
    // Blog
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub reading_time: Option<u32>,
    #[serde(default)]
    pub featured: bool,
    #[serde(default)]
    pub cover_image: Option<String>,
    #[serde(default)]
    pub cover_alt: Option<String>,
    // Project
    #[serde(default)]
    pub tech_stack: Vec<String>,
    #[serde(default)]
    pub repo_url: Option<String>,
    #[serde(default)]
    pub live_url: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    // Skills
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub level: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    // Navigation
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub external: bool,
    #[serde(default)]
    pub target_page: Option<String>,
    // Timeline
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub location: Option<String>,
    // Social
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub handle: Option<String>,
    // Hero
    #[serde(default)]
    pub headline: Option<String>,
    #[serde(default)]
    pub sub_headline: Option<String>,
    #[serde(default)]
    pub cta_label: Option<String>,
    #[serde(default)]
    pub cta_url: Option<String>,
    #[serde(default)]
    pub background: Option<String>,
    // Stats
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub suffix: Option<String>,
    // SEO
    #[serde(default)]
    pub canonical: Option<String>,
    #[serde(default)]
    pub og_image: Option<String>,
    #[serde(default)]
    pub no_index: bool,
    // Layout
    #[serde(default)]
    pub layout: Option<String>,
}

impl Frontmatter {
    pub fn slot_assignment(&self) -> SiteResult<SlotAssignment> {
        self.slot
            .parse::<SlotType>()
            .ok()
            .ok_or_else(|| SiteError::UnknownSlot {
                slot: self.slot.clone(),
                path: "<unknown>".into(),
            })
            .map(|slot_type| SlotAssignment {
                slot_type,
                order: self.order,
                weight: self.weight,
                page_scope: self.page_scope.clone(),
            })
    }

    pub fn resolve_slug(&self) -> String {
        self.slug
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| slug::slugify(&self.title))
    }
}

// ── Raw document ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RawDocument {
    pub frontmatter: Frontmatter,
    pub body: String,
    pub source_path: String,
}

// ── Parser ─────────────────────────────────────────────────────────────────────

/// Split raw source into (yaml_str, body_str) — pure function.
pub fn split_frontmatter(source: &str) -> SiteResult<(&str, &str)> {
    let trimmed = source.trim_start();
    if !trimmed.starts_with("---") {
        return Err(SiteError::Frontmatter {
            path: "<input>".into(),
            message: "Does not start with '---'".into(),
        });
    }
    let after = trimmed["---".len()..].trim_start_matches('\n');
    let close = after
        .find("\n---")
        .or_else(|| after.find("\n..."))
        .ok_or_else(|| SiteError::Frontmatter {
            path: "<input>".into(),
            message: "Closing '---' not found".into(),
        })?;
    let yaml = &after[..close];
    let rest = &after[close + "\n---".len()..];
    let body = rest
        .find('\n')
        .map(|n| rest[n..].trim_start_matches('\n'))
        .or_else(|| (!rest.trim().is_empty()).then(|| rest.trim_start_matches('\n')))
        .unwrap_or("");
    Ok((yaml, body))
}

/// Parse a YAML frontmatter string into Frontmatter using a simple TOML-compatible approach.
/// We convert the YAML to a serde_json::Value via a hand-rolled parser for simple key:value,
/// then deserialise into Frontmatter.
pub fn parse_document(source: &str, source_path: &str) -> SiteResult<RawDocument> {
    let (yaml, body) = split_frontmatter(source).map_err(|e| match e {
        SiteError::Frontmatter { message, .. } => SiteError::Frontmatter {
            path: source_path.to_string(),
            message,
        },
        other => other,
    })?;

    // Parse via TOML (YAML simple subset is TOML-compatible for our frontmatter style)
    let fm: Frontmatter = toml::from_str(yaml).map_err(|e| SiteError::Frontmatter {
        path: source_path.to_string(),
        message: e.to_string(),
    })?;

    Ok(RawDocument {
        frontmatter: fm,
        body: body.to_string(),
        source_path: source_path.to_string(),
    })
}

// ── Helpers ────────────────────────────────────────────────────────────────────

pub fn estimate_reading_time(markdown: &str) -> u32 {
    (markdown.split_whitespace().count() / 200).max(1) as u32
}

fn default_weight() -> i32 {
    50
}
fn default_star() -> String {
    "*".into()
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_frontmatter() {
        let doc = "---\ntitle = \"Hello World\"\nslot = \"article-card\"\n---\nBody.\n";
        let result = parse_document(doc, "test.md");
        assert!(result.is_ok(), "{:?}", result);
        let raw = result.unwrap();
        assert_eq!(raw.frontmatter.title, "Hello World");
        assert_eq!(raw.body.trim(), "Body.");
    }

    #[test]
    fn rejects_missing_delimiter() {
        assert!(parse_document("title = \"x\"\n\nBody.", "t.md").is_err());
    }

    #[test]
    fn slug_from_title() {
        let fm = Frontmatter {
            title: "My Cool Post".into(),
            slot: "article-card".into(),
            ..Default::default()
        };
        assert_eq!(fm.resolve_slug(), "my-cool-post");
    }

    #[test]
    fn explicit_slug_takes_precedence_when_present() {
        let fm = Frontmatter {
            title: "Ignored Title".into(),
            slug: Some("custom-slug".into()),
            slot: "article-card".into(),
            ..Default::default()
        };

        assert_eq!(fm.resolve_slug(), "custom-slug");
    }

    #[test]
    fn split_frontmatter_supports_dot_terminator() {
        let source = "---\ntitle = \"Hello\"\nslot = \"article-card\"\n...\nBody\n";
        let (yaml, body) = split_frontmatter(source).expect("frontmatter should split");

        assert!(yaml.contains("title = \"Hello\""));
        assert_eq!(body, "Body\n");
    }

    #[test]
    fn slot_assignment_preserves_order_weight_and_scope() {
        let fm = Frontmatter {
            title: "Contact".into(),
            slot: "contact-form".into(),
            order: 3,
            weight: 90,
            page_scope: "contact".into(),
            ..Default::default()
        };

        let assignment = fm.slot_assignment().expect("slot assignment should parse");

        assert_eq!(assignment.slot_type, SlotType::ContactForm);
        assert_eq!(assignment.order, 3);
        assert_eq!(assignment.weight, 90);
        assert_eq!(assignment.page_scope, "contact");
    }

    #[test]
    fn estimate_reading_time_has_one_minute_floor() {
        assert_eq!(estimate_reading_time("tiny post"), 1);
    }
}
