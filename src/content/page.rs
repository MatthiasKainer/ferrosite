use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::content::article::Article;
use crate::content::slot::SlotType;

// ── Page Types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageType {
    Home,
    Blog,
    Post,
    About,
    Contact,
    Projects,
    Project,
    Custom(String),
}

impl PageType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Home => "home",
            Self::Blog => "blog",
            Self::Post => "post",
            Self::About => "about",
            Self::Contact => "contact",
            Self::Projects => "projects",
            Self::Project => "project",
            Self::Custom(s) => s.as_str(),
        }
    }

    pub fn layout_name(&self) -> &str {
        match self {
            Self::Home => "home",
            Self::Blog => "blog",
            Self::Post => "post",
            Self::About => "about",
            Self::Contact => "contact",
            Self::Projects => "projects",
            Self::Project => "post", // projects use post-style layout
            Self::Custom(s) => s.as_str(),
        }
    }

    pub fn url_path(&self, slug: Option<&str>) -> String {
        match self {
            Self::Home => "/".to_string(),
            Self::Blog => "/blog/".to_string(),
            Self::Post => format!("/blog/{}/", slug.unwrap_or("post")),
            Self::About => "/about/".to_string(),
            Self::Contact => "/contact/".to_string(),
            Self::Projects => "/projects/".to_string(),
            Self::Project => format!("/projects/{}/", slug.unwrap_or("project")),
            Self::Custom(name) => format!("/{}/", name),
        }
    }
}

// ── SlotMap ────────────────────────────────────────────────────────────────────

/// A mapping from SlotType → ordered list of Articles filling that slot.
/// This is the primary data structure for template rendering.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SlotMap(pub HashMap<String, Vec<Article>>);

impl SlotMap {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Insert an article into its slot, maintaining sort order — pure wrt map state.
    pub fn insert(&mut self, slot: SlotType, article: Article) {
        let key = slot.to_string();
        let vec = self.0.entry(key).or_default();
        vec.push(article);
        vec.sort_by(|a, b| compare_articles_for_slot(&slot, a, b));
    }

    /// Get all articles in a slot — pure function.
    pub fn get(&self, slot: &SlotType) -> &[Article] {
        self.0
            .get(&slot.to_string())
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get the first article in a slot (for single-occupancy slots) — pure.
    pub fn get_first(&self, slot: &SlotType) -> Option<&Article> {
        self.get(slot).first()
    }

    /// Check whether a slot has any content — pure.
    pub fn has(&self, slot: &SlotType) -> bool {
        !self.get(slot).is_empty()
    }

    /// Merge another SlotMap into this one — pure accumulation.
    pub fn merge(mut self, other: SlotMap) -> Self {
        for (key, articles) in other.0 {
            if let Ok(st) = key.parse::<SlotType>() {
                for a in articles {
                    self.insert(st.clone(), a);
                }
            }
        }
        self
    }

    /// Serialize to JSON value for template context — pure function.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.0).unwrap_or(serde_json::Value::Object(Default::default()))
    }
}

fn compare_articles_for_slot(slot: &SlotType, left: &Article, right: &Article) -> Ordering {
    let prefers_date_desc = matches!(slot, SlotType::ArticleCard | SlotType::ArticleBody);

    if prefers_date_desc {
        let left_date = left.frontmatter.date.as_deref().unwrap_or("");
        let right_date = right.frontmatter.date.as_deref().unwrap_or("");
        match right_date.cmp(left_date) {
            Ordering::Equal => {}
            non_equal => return non_equal,
        }
    }

    left.sort_key().cmp(&right.sort_key())
}

// ── Page ───────────────────────────────────────────────────────────────────────

/// A fully assembled page, ready for template rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub page_type: PageType,
    pub title: String,
    pub description: String,
    pub url_path: String,
    /// Slug for single-content pages (posts, projects)
    pub slug: Option<String>,
    /// All slot content for this page
    pub slots: SlotMap,
    /// The primary article for post/project pages
    pub primary_article: Option<Article>,
    /// Page-specific metadata for SEO
    pub meta: PageMeta,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PageMeta {
    pub og_title: String,
    pub og_description: String,
    pub og_image: Option<String>,
    pub canonical: Option<String>,
    pub no_index: bool,
    pub extra_head: Vec<String>,
}

impl Page {
    /// Create a page from type and site globals — pure construction.
    pub fn new(page_type: PageType, title: String, description: String, slots: SlotMap) -> Self {
        let url_path = page_type.url_path(None);
        let meta = PageMeta {
            og_title: title.clone(),
            og_description: description.clone(),
            ..Default::default()
        };
        Self {
            page_type,
            title,
            description,
            url_path,
            slug: None,
            slots,
            primary_article: None,
            meta,
        }
    }

    /// Create a post/project page — pure construction.
    pub fn from_article(
        page_type: PageType,
        article: Article,
        global_slots: SlotMap,
        base_url: &str,
    ) -> Self {
        let slug = article.frontmatter.resolve_slug();
        let url_path = article_relative_url_path(&article.url_path, base_url)
            .unwrap_or_else(|| page_type.url_path(Some(&slug)));
        let description = article.excerpt.clone();
        let title = article.frontmatter.title.clone();
        let meta = PageMeta {
            og_title: title.clone(),
            og_description: description.clone(),
            og_image: article
                .frontmatter
                .og_image
                .clone()
                .or_else(|| article.frontmatter.cover_image.clone()),
            canonical: article.frontmatter.canonical.clone(),
            no_index: article.frontmatter.no_index,
            ..Default::default()
        };
        Self {
            page_type,
            title,
            description,
            url_path,
            slug: Some(slug),
            slots: global_slots,
            primary_article: Some(article),
            meta,
        }
    }

    /// Build the template rendering context as a JSON value — pure function.
    pub fn to_render_context(&self, site_json: &serde_json::Value) -> serde_json::Value {
        use serde_json::json;

        json!({
            "site": site_json,
            "page": {
                "type": self.page_type.as_str(),
                "title": self.title,
                "description": self.description,
                "url_path": self.url_path,
                "slug": self.slug,
                "meta": self.meta,
            },
            "slots": self.slots.to_json(),
            "article": self.primary_article,
        })
    }
}

fn article_relative_url_path(article_url: &str, base_url: &str) -> Option<String> {
    let base = base_url.trim_end_matches('/');
    article_url.strip_prefix(base).map(|path| {
        if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        }
    })
}

// ── Page collection ────────────────────────────────────────────────────────────

/// The set of all pages to be generated for a site — pure data.
#[derive(Debug, Clone, Default)]
pub struct PageCollection {
    pub pages: Vec<Page>,
}

impl PageCollection {
    pub fn new(pages: Vec<Page>) -> Self {
        Self { pages }
    }

    /// Filter pages by type — pure function.
    pub fn of_type(&self, page_type: &PageType) -> Vec<&Page> {
        self.pages
            .iter()
            .filter(|p| &p.page_type == page_type)
            .collect()
    }

    /// Total page count.
    pub fn len(&self) -> usize {
        self.pages.len()
    }
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::frontmatter::Frontmatter;

    fn article(title: &str, order: i32, weight: i32) -> Article {
        Article {
            id: slug::slugify(title),
            frontmatter: Frontmatter {
                title: title.into(),
                slot: "article-card".into(),
                order,
                weight,
                ..Default::default()
            },
            markdown_body: String::new(),
            html_body: String::new(),
            url_path: format!("/blog/{}/", slug::slugify(title)),
            source_path: format!("blog/{}.md", slug::slugify(title)),
            reading_time: 1,
            excerpt: format!("{title} excerpt"),
        }
    }

    #[test]
    fn page_type_uses_expected_layouts_and_routes() {
        assert_eq!(PageType::Project.layout_name(), "post");
        assert_eq!(
            PageType::Project.url_path(Some("cli-tool")),
            "/projects/cli-tool/"
        );
        assert_eq!(PageType::Custom("lab".into()).url_path(None), "/lab/");
    }

    #[test]
    fn slot_map_keeps_articles_sorted_by_order_weight_and_title() {
        let mut slots = SlotMap::new();
        slots.insert(SlotType::ArticleCard, article("Gamma", 2, 50));
        slots.insert(SlotType::ArticleCard, article("Alpha", 1, 40));
        slots.insert(SlotType::ArticleCard, article("Beta", 1, 60));

        let titles: Vec<_> = slots
            .get(&SlotType::ArticleCard)
            .iter()
            .map(|article| article.frontmatter.title.as_str())
            .collect();

        assert_eq!(titles, vec!["Beta", "Alpha", "Gamma"]);
    }

    #[test]
    fn page_from_article_carries_primary_article_and_seo_metadata() {
        let mut article = article("Portable CLI", 0, 50);
        article.frontmatter.slot = "project-body".into();
        article.frontmatter.cover_image = Some("/cover.png".into());
        article.frontmatter.og_image = Some("/og.png".into());
        article.frontmatter.canonical = Some("https://example.com/projects/portable-cli/".into());
        article.frontmatter.no_index = true;

        let page = Page::from_article(
            PageType::Project,
            article.clone(),
            SlotMap::new(),
            "https://example.com",
        );

        assert_eq!(page.title, "Portable CLI");
        assert_eq!(page.slug.as_deref(), Some("portable-cli"));
        assert_eq!(page.url_path, "/projects/portable-cli/");
        assert_eq!(page.meta.og_image.as_deref(), Some("/og.png"));
        assert_eq!(
            page.meta.canonical.as_deref(),
            Some("https://example.com/projects/portable-cli/")
        );
        assert!(page.meta.no_index);
        assert_eq!(
            page.primary_article.as_ref().map(|a| a.id.as_str()),
            Some("portable-cli")
        );
    }

    #[test]
    fn render_context_exposes_site_page_and_slot_data() {
        let mut slots = SlotMap::new();
        slots.insert(SlotType::ArticleCard, article("Alpha", 1, 50));
        let page = Page::new(
            PageType::Blog,
            "Blog".into(),
            "All posts".into(),
            slots.clone(),
        );
        let site_json = serde_json::json!({ "title": "Ferrosite" });

        let context = page.to_render_context(&site_json);

        assert_eq!(context["site"]["title"], "Ferrosite");
        assert_eq!(context["page"]["type"], "blog");
        assert_eq!(context["page"]["url_path"], "/blog/");
        assert_eq!(
            context["slots"]["article-card"][0]["frontmatter"]["title"],
            "Alpha"
        );
    }
}
