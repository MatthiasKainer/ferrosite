use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::config::RouteConfig;
use crate::content::frontmatter::{estimate_reading_time, parse_document, Frontmatter};
use crate::content::slot::SlotType;
use crate::error::{io_with_path, SiteError, SiteResult};

// ── Article ────────────────────────────────────────────────────────────────────

/// A fully parsed article ready for slot assignment and rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: String,
    pub frontmatter: Frontmatter,
    /// Raw markdown body
    pub markdown_body: String,
    /// Rendered HTML body (populated after markdown → HTML step)
    pub html_body: String,
    /// Canonical URL path (e.g. "/blog/hello-world/")
    pub url_path: String,
    /// Source file relative path
    pub source_path: String,
    /// Resolved reading time in minutes
    pub reading_time: u32,
    /// Excerpt — first paragraph or explicit description
    pub excerpt: String,
}

impl Article {
    /// Build an Article from a raw markdown file path — side effect: reads file.
    pub fn from_file(
        file_path: &Path,
        content_root: &Path,
        base_url: &str,
        routes: &RouteConfig,
    ) -> SiteResult<Self> {
        let source = std::fs::read_to_string(file_path)
            .map_err(io_with_path(file_path, "reading content file"))?;
        let rel_path = file_path
            .strip_prefix(content_root)?
            .to_string_lossy()
            .into_owned();

        Self::from_source(&source, &rel_path, base_url, routes)
    }

    /// Build an Article from source text — pure after parsing side effect.
    pub fn from_source(
        source: &str,
        rel_path: &str,
        base_url: &str,
        routes: &RouteConfig,
    ) -> SiteResult<Self> {
        let raw = parse_document(source, rel_path)?;
        Self::from_raw_document(raw.frontmatter, raw.body, rel_path, base_url, routes)
    }

    /// Construct Article from already-parsed parts — pure function.
    pub fn from_raw_document(
        frontmatter: Frontmatter,
        markdown_body: String,
        source_path: &str,
        base_url: &str,
        routes: &RouteConfig,
    ) -> SiteResult<Self> {
        let slug = frontmatter.resolve_slug();
        let slot_type =
            frontmatter
                .slot
                .parse::<SlotType>()
                .ok()
                .ok_or_else(|| SiteError::UnknownSlot {
                    slot: frontmatter.slot.clone(),
                    path: source_path.to_string(),
                })?;

        let url_path = build_url_path(source_path, &slug, &slot_type, base_url, routes);
        let html_body = render_markdown(&markdown_body);
        let reading_time = frontmatter
            .reading_time
            .unwrap_or_else(|| estimate_reading_time(&markdown_body));
        let excerpt = frontmatter
            .description
            .clone()
            .unwrap_or_else(|| extract_excerpt(&markdown_body, 200));
        let id = slug.clone();

        Ok(Self {
            id,
            frontmatter,
            markdown_body,
            html_body,
            url_path,
            source_path: source_path.to_string(),
            reading_time,
            excerpt,
        })
    }

    /// Whether this article is published (not a draft).
    pub fn is_published(&self) -> bool {
        !self.frontmatter.draft
    }

    /// The slot type this article targets.
    pub fn slot_type(&self) -> SiteResult<SlotType> {
        self.frontmatter
            .slot
            .parse::<SlotType>()
            .ok()
            .ok_or_else(|| SiteError::UnknownSlot {
                slot: self.frontmatter.slot.clone(),
                path: self.source_path.clone(),
            })
    }

    /// Sort key: (order, -weight, title)
    pub fn sort_key(&self) -> (i32, i32, &str) {
        (
            self.frontmatter.order,
            -self.frontmatter.weight,
            &self.frontmatter.title,
        )
    }
}

// ── Markdown rendering ─────────────────────────────────────────────────────────

/// Convert markdown to HTML — pure function.
pub fn render_markdown(markdown: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};

    let options = Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_SMART_PUNCTUATION;

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

// ── URL routing ────────────────────────────────────────────────────────────────

/// Derive a canonical URL path from source path + slug + slot type — pure function.
///
/// Routing rules:
///   - blog/* → /blog/{slug}/
///   - projects/* → /projects/{slug}/
///   - article-body slot → /blog/{slug}/
///   - project-body slot → /projects/{slug}/
///   - about-body slot → /about/
///   - everything else → /{slug}/ (top-level)
fn build_url_path(
    source_path: &str,
    slug: &str,
    slot: &SlotType,
    base_url: &str,
    routes: &RouteConfig,
) -> String {
    let base = base_url.trim_end_matches('/');
    let path_lower = source_path.to_lowercase();
    let blog_post_path = normalize_route_prefix(&routes.blog_post_path);
    let projects_path = normalize_route_prefix(&routes.projects_path);

    let rel = if path_lower.contains("blog/") || *slot == SlotType::ArticleBody {
        format!("/{}/{}/", blog_post_path, slug)
    } else if path_lower.contains("project") || *slot == SlotType::ProjectBody {
        format!("/{}/{}/", projects_path, slug)
    } else if *slot == SlotType::AboutBody {
        "/about/".to_string()
    } else if *slot == SlotType::ContactForm {
        "/contact/".to_string()
    } else {
        format!("/{}/", slug)
    };

    format!("{}{}", base, rel)
}

fn normalize_route_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim().trim_matches('/');
    if trimmed.is_empty() {
        "blog".into()
    } else {
        trimmed.into()
    }
}

// ── Excerpt ────────────────────────────────────────────────────────────────────

/// Extract first `max_chars` of plaintext from markdown — pure function.
pub fn extract_excerpt(markdown: &str, max_chars: usize) -> String {
    // Skip frontmatter, headings, code blocks; take first paragraph text
    let plain: String = markdown
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.starts_with('#') && !t.starts_with("```") && !t.starts_with("---") && !t.is_empty()
        })
        .take(5)
        .collect::<Vec<_>>()
        .join(" ");

    // Strip markdown inline syntax
    let cleaned = plain
        .replace("**", "")
        .replace(['*', '`'], "")
        .replace("__", "")
        .replace('_', "");

    if cleaned.len() <= max_chars {
        cleaned
    } else {
        let truncated = &cleaned[..max_chars];
        let last_space = truncated.rfind(' ').unwrap_or(max_chars);
        format!("{}…", &truncated[..last_space])
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_markdown_to_html() {
        let html = render_markdown("# Hello\n\nWorld **bold**");
        assert!(html.contains("<h1"));
        assert!(html.contains("<strong>bold</strong>"));
    }

    #[test]
    fn extracts_excerpt() {
        let md = "# Title\n\nThis is the first paragraph with some content.\n\nSecond paragraph.";
        let excerpt = extract_excerpt(md, 50);
        assert!(!excerpt.is_empty());
        assert!(!excerpt.contains('#'));
    }

    #[test]
    fn from_raw_document_builds_blog_article_metadata() {
        let frontmatter = Frontmatter {
            title: "Hello Rust".into(),
            slot: "article-body".into(),
            description: Some("Purpose-built excerpt".into()),
            reading_time: Some(7),
            ..Default::default()
        };

        let article = Article::from_raw_document(
            frontmatter,
            "Intro paragraph\n\nMore text.".into(),
            "blog/hello-rust.md",
            "https://example.com/",
            &RouteConfig::default(),
        )
        .expect("article should build");

        assert_eq!(article.id, "hello-rust");
        assert_eq!(article.url_path, "https://example.com/blog/hello-rust/");
        assert_eq!(article.reading_time, 7);
        assert_eq!(article.excerpt, "Purpose-built excerpt");
        assert!(article.html_body.contains("<p>Intro paragraph</p>"));
    }

    #[test]
    fn from_raw_document_uses_slot_routing_for_project_pages() {
        let frontmatter = Frontmatter {
            title: "CLI Tool".into(),
            slot: "project-body".into(),
            ..Default::default()
        };

        let article = Article::from_raw_document(
            frontmatter,
            "Project body".into(),
            "work/cli-tool.md",
            "https://example.com",
            &RouteConfig::default(),
        )
        .expect("article should build");

        assert_eq!(article.url_path, "https://example.com/projects/cli-tool/");
    }

    #[test]
    fn draft_articles_build_but_are_not_published() {
        let frontmatter = Frontmatter {
            title: "Hidden".into(),
            slot: "article-body".into(),
            draft: true,
            ..Default::default()
        };

        let article = Article::from_raw_document(
            frontmatter,
            "Secret".into(),
            "blog/hidden.md",
            "https://example.com",
            &RouteConfig::default(),
        )
        .expect("drafts should still be routable");

        assert_eq!(article.url_path, "https://example.com/blog/hidden/");
        assert!(!article.is_published());
    }

    #[test]
    fn excerpt_truncates_on_word_boundary_and_strips_inline_markdown() {
        let md = "This paragraph has **bold** words and `inline code` that should be cleaned.";

        let excerpt = extract_excerpt(md, 35);

        assert_eq!(excerpt, "This paragraph has bold words and…");
    }
}
