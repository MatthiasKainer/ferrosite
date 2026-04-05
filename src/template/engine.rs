use minijinja::{Environment, Value};
use serde_json::json;
use std::path::Path;
use walkdir::WalkDir;

use crate::config::{SiteConfig, ThemeConfig};
use crate::content::page::Page;
use crate::error::{io_with_path, SiteError, SiteResult};

// ── Template Engine ────────────────────────────────────────────────────────────

/// A loaded template environment bound to a specific template directory.
pub struct TemplateEngine {
    env: Environment<'static>,
    template_dir: std::path::PathBuf,
}

impl TemplateEngine {
    /// Load all layout templates from a directory — side effect: reads files.
    pub fn from_dir(template_dir: &Path) -> SiteResult<Self> {
        let layouts_dir = template_dir.join("layouts");
        if !layouts_dir.exists() {
            return Err(SiteError::TemplateNotFound {
                template: layouts_dir.display().to_string(),
            });
        }

        let mut env = Environment::new();

        // Load all .html files from the layouts directory
        for entry in WalkDir::new(&layouts_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "html"))
        {
            let path = entry.path();
            let name = path
                .strip_prefix(&layouts_dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let content = std::fs::read_to_string(path)
                .map_err(io_with_path(path, "reading template file"))?;
            env.add_template_owned(name, content)
                .map_err(|e| SiteError::TemplateEngine { source: e })?;
        }

        Ok(Self {
            env,
            template_dir: template_dir.to_path_buf(),
        })
    }

    /// Render a page to HTML — pure-ish (reads from in-memory env).
    pub fn render_page(
        &self,
        page: &Page,
        site_config: &SiteConfig,
        theme: &ThemeConfig,
        extra_head: &str,
    ) -> SiteResult<String> {
        let layout = format!("{}.html", page.page_type.layout_name());
        let template = self
            .env
            .get_template(&layout)
            .or_else(|_| self.env.get_template("base.html"))
            .map_err(|_| SiteError::LayoutNotFound {
                layout: layout.clone(),
                page_type: page.page_type.as_str().to_string(),
            })?;

        let site_json = site_to_json(site_config);
        let theme_json = serde_json::to_value(theme).unwrap_or_default();
        let mut ctx = page.to_render_context(&site_json);

        // Inject theme and extra head
        if let serde_json::Value::Object(ref mut m) = ctx {
            m.insert("theme".into(), theme_json);
            m.insert("extra_head".into(), json!(extra_head));
        }

        let ctx_value = Value::from_serializable(&ctx);
        template.render(ctx_value).map_err(SiteError::from)
    }

    /// Render a named partial from the layouts dir — pure-ish.
    pub fn render_partial(&self, name: &str, ctx: &serde_json::Value) -> SiteResult<String> {
        let template_name = if name.ends_with(".html") {
            name.to_string()
        } else {
            format!("{}.html", name)
        };
        let template =
            self.env
                .get_template(&template_name)
                .map_err(|_| SiteError::TemplateNotFound {
                    template: template_name.clone(),
                })?;
        template
            .render(Value::from_serializable(ctx))
            .map_err(SiteError::from)
    }

    pub fn template_dir(&self) -> &Path {
        &self.template_dir
    }
}

// ── Context helpers ────────────────────────────────────────────────────────────

/// Convert SiteConfig to a JSON Value for template context — pure function.
pub fn site_to_json(config: &SiteConfig) -> serde_json::Value {
    json!({
        "title": config.site.title,
        "description": config.site.description,
        "base_url": config.site.base_url,
        "author": {
            "name": config.site.author.name,
            "email": config.site.author.email,
            "bio": config.site.author.bio,
            "avatar": config.site.author.avatar,
        },
        "language": config.site.language,
        "keywords": config.site.keywords,
        "favicon": config.site.favicon,
        "social": {
            "github": config.site.social.github,
            "linkedin": config.site.social.linkedin,
            "twitter": config.site.social.twitter,
            "mastodon": config.site.social.mastodon,
            "website": config.site.social.website,
        },
        "layout": {
            "menu": config.layout.menu,
            "dock": config.layout.dock,
            "sidebar": config.layout.sidebar,
            "sidebar_position": config.layout.sidebar_position,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        AuthorConfig, BuildConfig, DeployConfig, DeployProvider, LayoutConfig, RouteConfig,
        SiteMetadata, SocialConfig,
    };
    use std::collections::HashMap;

    #[test]
    fn site_to_json_exposes_template_relevant_site_metadata() {
        let config = SiteConfig {
            site: SiteMetadata {
                title: "Ferrosite".into(),
                description: "A static site".into(),
                base_url: "https://example.com".into(),
                author: AuthorConfig {
                    name: "Matthias".into(),
                    email: Some("matthias@example.com".into()),
                    ..Default::default()
                },
                language: "en".into(),
                keywords: vec!["rust".into(), "static-site".into()],
                favicon: Some("/favicon.ico".into()),
                social: SocialConfig {
                    github: Some("matthiaskainer".into()),
                    ..Default::default()
                },
            },
            build: BuildConfig::default(),
            deploy: DeployConfig {
                provider: DeployProvider::Cloudflare,
                cloudflare: None,
                aws: None,
                azure: None,
            },
            layout: LayoutConfig {
                menu: true,
                dock: true,
                sidebar: true,
                sidebar_position: "left".into(),
            },
            routes: RouteConfig::default(),
            plugins: Default::default(),
            extra: HashMap::new(),
        };

        let site = site_to_json(&config);

        assert_eq!(site["title"], "Ferrosite");
        assert_eq!(site["author"]["email"], "matthias@example.com");
        assert_eq!(site["social"]["github"], "matthiaskainer");
        assert_eq!(site["layout"]["sidebar_position"], "left");
        assert_eq!(site["keywords"][1], "static-site");
    }
}
