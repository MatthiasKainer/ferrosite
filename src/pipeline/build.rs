use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

use crate::config::{
    load_site_config_for_root, load_theme_config, template_dir, theme_to_css_vars, SiteConfig,
    ThemeConfig,
};
use crate::content::article::Article;
use crate::content::page::{Page, PageCollection, PageType, SlotMap};
use crate::content::slot::SlotType;
use crate::error::{SiteError, SiteResult};
use crate::pipeline::media::{prepare_media_plan, write_media_assets};
use crate::plugin::PluginRegistry;
use crate::template::{ComponentRegistry, TemplateEngine};

const PFUSCH_CDN: &str = "https://cdn.jsdelivr.net/gh/MatthiasKainer/pfusch@main/pfusch.js";

// ── Build Context ──────────────────────────────────────────────────────────────

/// All data needed for a site build, loaded once and shared — immutable after construction.
pub struct BuildContext {
    pub site_root: PathBuf,
    pub config: SiteConfig,
    pub theme: ThemeConfig,
    pub engine: TemplateEngine,
    pub components: ComponentRegistry,
    pub plugins: PluginRegistry,
}

impl BuildContext {
    /// Load a full BuildContext from a site root directory — side effects: reads files.
    pub fn load(site_root: &Path) -> SiteResult<Self> {
        let config = load_site_config_for_root(site_root)?;

        let tmpl_dir = template_dir(site_root, &config.build.template);
        let theme_path = tmpl_dir.join("theme.toml");
        let theme = load_theme_config(&theme_path)?;

        let engine = TemplateEngine::from_dir(&tmpl_dir)?;
        let components_dir = tmpl_dir.join("components");
        let components = ComponentRegistry::load_from_dir(&components_dir, PFUSCH_CDN)?;

        let plugins_dir = config
            .plugins
            .plugins_dir
            .as_deref()
            .map(|d| site_root.join(d))
            .unwrap_or_else(|| site_root.join("plugins"));

        let plugins = PluginRegistry::load_from_dir(&plugins_dir, &config.plugins.enabled)?;
        plugins.validate_no_conflicts()?;

        let components = components.with_plugin_components(plugins.component_defs());

        Ok(Self {
            site_root: site_root.to_path_buf(),
            config,
            theme,
            engine,
            components,
            plugins,
        })
    }
}

// ── Step 1: Collect articles ───────────────────────────────────────────────────

/// Collect all articles from the content directory — side effect: reads files.
pub fn collect_articles(ctx: &BuildContext) -> SiteResult<Vec<Article>> {
    let content_dir = ctx.site_root.join(&ctx.config.build.content_dir);

    // Also check template-bundled content
    let tmpl_dir = template_dir(&ctx.site_root, &ctx.config.build.template);
    let tmpl_content_dir = tmpl_dir.join("content");

    let mut all_results = if content_dir.exists() {
        collect_articles_from_dir(
            &content_dir,
            &content_dir,
            &ctx.config.site.base_url,
            &ctx.config.routes,
        )
    } else {
        Vec::new()
    };

    // Load template defaults unless they resolve to the same content dir.
    if tmpl_content_dir.exists() && !same_existing_dir(&content_dir, &tmpl_content_dir) {
        let site_source_paths: HashSet<String> = all_results
            .iter()
            .filter_map(|result| result.as_ref().ok())
            .map(|article| article.source_path.clone())
            .collect();

        all_results.extend(
            collect_articles_from_dir(
                &tmpl_content_dir,
                &tmpl_content_dir,
                &ctx.config.site.base_url,
                &ctx.config.routes,
            )
            .into_iter()
            // Site content overrides template defaults with the same relative path.
            .filter(|result| match result {
                Ok(article) => !site_source_paths.contains(&article.source_path),
                Err(_) => true,
            }),
        );
    }

    // Partition successes and errors
    let (ok, errs): (Vec<_>, Vec<_>) = all_results.into_iter().partition(|r| r.is_ok());

    if !errs.is_empty() {
        eprintln!("Warning: {} article(s) had errors:", errs.len());
        for e in errs.iter().map(|r| r.as_ref().unwrap_err()) {
            eprintln!("  - {}", e);
        }
    }

    Ok(ok.into_iter().map(|r| r.unwrap()).collect())
}

fn collect_articles_from_dir(
    dir: &Path,
    root: &Path,
    base_url: &str,
    routes: &crate::config::RouteConfig,
) -> Vec<SiteResult<Article>> {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().is_some_and(|ext| ext == "md") && e.file_name() != "_index.md"
        })
        .map(|entry| Article::from_file(entry.path(), root, base_url, routes))
        .collect()
}

fn same_existing_dir(a: &Path, b: &Path) -> bool {
    if !a.exists() || !b.exists() {
        return false;
    }

    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

// ── Step 2: Build global SlotMap ───────────────────────────────────────────────

/// Partition articles into a global SlotMap — pure function.
///
/// Articles with `page_scope = "*"` or layout region slots go into the global map
/// (shared across all pages). Articles scoped to a specific page type go into
/// page-specific maps built later.
pub fn build_global_slot_map(articles: &[Article]) -> SiteResult<SlotMap> {
    let mut global = SlotMap::new();

    for article in articles {
        let slot_type = article.slot_type()?;
        let scope = &article.frontmatter.page_scope;

        // Global slots: layout regions are always global
        let is_layout_region = matches!(
            slot_type,
            SlotType::HeaderBrand
                | SlotType::HeaderAction
                | SlotType::FooterAbout
                | SlotType::FooterNavColumn
                | SlotType::FooterBottom
                | SlotType::NavItem
                | SlotType::DockItem
                | SlotType::SocialLink
        );

        if scope == "*" || is_layout_region {
            global.insert(slot_type, article.clone());
        }
    }

    Ok(global)
}

// ── Step 3: Assemble pages ─────────────────────────────────────────────────────

/// Assemble all pages for the site from articles and global slots — pure function.
pub fn assemble_pages(
    articles: &[Article],
    global_slots: &SlotMap,
    config: &SiteConfig,
) -> SiteResult<PageCollection> {
    let mut pages = Vec::new();

    // ── Home page ──────────────────────────────────────────────────────
    pages.push(assemble_home_page(articles, global_slots, config)?);

    // ── Blog listing page ──────────────────────────────────────────────
    let blog_posts: Vec<&Article> = articles
        .iter()
        .filter(|a| {
            matches!(
                a.slot_type().ok().as_ref(),
                Some(SlotType::ArticleBody) | Some(SlotType::ArticleCard)
            ) || a.source_path.contains("blog/")
        })
        .collect();

    let published_blog_posts: Vec<&Article> = blog_posts
        .iter()
        .copied()
        .filter(|article| article.is_published())
        .collect();

    if !blog_posts.is_empty() {
        pages.push(assemble_blog_page(
            &published_blog_posts,
            global_slots,
            config,
        )?);

        // Individual post pages
        for post in &blog_posts {
            if post.slot_type().ok() == Some(SlotType::ArticleBody) {
                pages.push(Page::from_article(
                    PageType::Post,
                    (*post).clone(),
                    global_slots.clone(),
                    &config.site.base_url,
                ));
            }
        }
    }

    // ── About page ────────────────────────────────────────────────────
    let about_articles: Vec<&Article> = articles
        .iter()
        .filter(|a| {
            a.frontmatter.page_scope == "about"
                || matches!(
                    a.slot_type().ok().as_ref(),
                    Some(SlotType::AboutBody)
                        | Some(SlotType::CareerTimeline)
                        | Some(SlotType::TimelineEntry)
                )
        })
        .collect();

    if !about_articles.is_empty() || has_about_body(articles) {
        pages.push(assemble_about_page(articles, global_slots, config)?);
    }

    // ── Contact page ──────────────────────────────────────────────────
    pages.push(assemble_contact_page(global_slots, config));

    // ── Projects listing page ──────────────────────────────────────────
    let project_articles: Vec<&Article> = articles
        .iter()
        .filter(|a| {
            a.source_path.contains("project")
                || matches!(
                    a.slot_type().ok().as_ref(),
                    Some(SlotType::ProjectBody) | Some(SlotType::ProjectCard)
                )
        })
        .collect();

    if !project_articles.is_empty() {
        pages.push(assemble_projects_page(
            &project_articles,
            global_slots,
            config,
        )?);

        for proj in &project_articles {
            if proj.slot_type().ok() == Some(SlotType::ProjectBody) {
                pages.push(Page::from_article(
                    PageType::Project,
                    (*proj).clone(),
                    global_slots.clone(),
                    &config.site.base_url,
                ));
            }
        }
    }

    Ok(PageCollection::new(pages))
}

fn assemble_home_page(
    articles: &[Article],
    global_slots: &SlotMap,
    config: &SiteConfig,
) -> SiteResult<Page> {
    let mut home_slots = global_slots.clone();

    for article in articles {
        let scope = &article.frontmatter.page_scope;
        if scope == "home" {
            if let Ok(slot_type) = article.slot_type() {
                if !matches!(
                    slot_type,
                    SlotType::HeaderBrand
                        | SlotType::HeaderAction
                        | SlotType::FooterAbout
                        | SlotType::FooterNavColumn
                        | SlotType::FooterBottom
                ) {
                    home_slots.insert(slot_type, article.clone());
                }
            }
        }
    }

    Ok(Page::new(
        PageType::Home,
        config.site.title.clone(),
        config.site.description.clone(),
        home_slots,
    ))
}

fn assemble_blog_page(
    posts: &[&Article],
    global_slots: &SlotMap,
    config: &SiteConfig,
) -> SiteResult<Page> {
    let mut blog_slots = global_slots.clone();

    for post in posts {
        blog_slots.insert(SlotType::ArticleCard, (*post).clone());
    }

    Ok(Page::new(
        PageType::Blog,
        format!("Blog — {}", config.site.title),
        format!("Articles and thoughts by {}", config.site.author.name),
        blog_slots,
    ))
}

fn assemble_about_page(
    articles: &[Article],
    global_slots: &SlotMap,
    config: &SiteConfig,
) -> SiteResult<Page> {
    let mut about_slots = global_slots.clone();

    for article in articles {
        let scope = &article.frontmatter.page_scope;
        if scope == "about" {
            if let Ok(slot_type) = article.slot_type() {
                about_slots.insert(slot_type, article.clone());
            }
        }
    }

    let about_body = articles
        .iter()
        .find(|a| a.slot_type().ok() == Some(SlotType::AboutBody));

    let description = about_body
        .map(|a| a.excerpt.clone())
        .unwrap_or_else(|| format!("About {}", config.site.author.name));

    Ok(Page::new(
        PageType::About,
        format!("About — {}", config.site.author.name),
        description,
        about_slots,
    ))
}

fn assemble_contact_page(global_slots: &SlotMap, config: &SiteConfig) -> Page {
    Page::new(
        PageType::Contact,
        format!("Contact — {}", config.site.author.name),
        format!("Get in touch with {}", config.site.author.name),
        global_slots.clone(),
    )
}

fn assemble_projects_page(
    projects: &[&Article],
    global_slots: &SlotMap,
    config: &SiteConfig,
) -> SiteResult<Page> {
    let mut proj_slots = global_slots.clone();

    for project in projects {
        proj_slots.insert(SlotType::ProjectCard, (*project).clone());
    }

    Ok(Page::new(
        PageType::Projects,
        format!("Projects — {}", config.site.author.name),
        "Portfolio of projects and work".into(),
        proj_slots,
    ))
}

fn has_about_body(articles: &[Article]) -> bool {
    articles
        .iter()
        .any(|a| a.slot_type().ok() == Some(SlotType::AboutBody))
}

// ── Step 4: Render pages ───────────────────────────────────────────────────────

/// Rendered output for a single page.
#[derive(Debug, Clone)]
pub struct RenderedPage {
    pub url_path: String,
    pub output_path: PathBuf,
    pub html: String,
}

/// Render all pages to HTML strings — pure-ish (reads from in-memory engine).
pub fn render_pages(pages: &PageCollection, ctx: &BuildContext) -> SiteResult<Vec<RenderedPage>> {
    let css_vars = theme_to_css_vars(&ctx.theme);
    let pfusch_style = crate::template::component::render_pfusch_style(&css_vars);
    let script_block = ctx.components.render_script_block();
    let plugin_head = ctx.plugins.all_head_injections();
    let extra_head = format!("{}\n{}\n{}", pfusch_style, script_block, plugin_head);

    pages
        .pages
        .iter()
        .map(|page| {
            let html = ctx
                .engine
                .render_page(page, &ctx.config, &ctx.theme, &extra_head)?;
            let output_path = url_to_output_path(&page.url_path);
            Ok(RenderedPage {
                url_path: page.url_path.clone(),
                output_path,
                html,
            })
        })
        .collect::<SiteResult<Vec<_>>>()
}

/// Convert a URL path to an output filesystem path — pure function.
///
/// `/blog/hello-world/` → `blog/hello-world/index.html`
/// `/` → `index.html`
fn url_to_output_path(url_path: &str) -> PathBuf {
    let trimmed = url_path.trim_start_matches('/');
    if trimmed.is_empty() {
        PathBuf::from("index.html")
    } else {
        PathBuf::from(trimmed).join("index.html")
    }
}

// ── Step 5: Write output ───────────────────────────────────────────────────────

/// Write rendered pages to the output directory — side effect: writes files.
pub fn write_output(rendered: &[RenderedPage], output_dir: &Path) -> SiteResult<Vec<PathBuf>> {
    rendered
        .iter()
        .map(|page| {
            let full_path = output_dir.join(&page.output_path);
            write_file_if_changed(&full_path, page.html.as_bytes())?;
            Ok(full_path)
        })
        .collect::<SiteResult<Vec<_>>>()
}

fn write_file_if_changed(path: &Path, contents: &[u8]) -> SiteResult<bool> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match std::fs::read(path) {
        Ok(existing) if existing == contents => Ok(false),
        Ok(_) | Err(_) => {
            std::fs::write(path, contents)?;
            Ok(true)
        }
    }
}

// ── Step 6: Copy assets ────────────────────────────────────────────────────────

/// Copy template and site assets to the output directory — side effect: copies files.
pub fn copy_assets(ctx: &BuildContext, output_dir: &Path) -> SiteResult<()> {
    let tmpl_dir = template_dir(&ctx.site_root, &ctx.config.build.template);
    let tmpl_assets = tmpl_dir.join(&ctx.config.build.assets_dir);
    let site_assets = ctx.site_root.join(&ctx.config.build.assets_dir);
    let out_assets = output_dir.join("assets");

    // Copy template assets first (lower priority)
    if tmpl_assets.exists() {
        copy_dir_all(&tmpl_assets, &out_assets)?;
    }

    // Copy site assets (higher priority, overwrites template assets)
    if site_assets.exists() {
        copy_dir_all(&site_assets, &out_assets)?;
    }

    Ok(())
}

fn copy_dir_all(src: &Path, dst: &Path) -> SiteResult<()> {
    std::fs::create_dir_all(dst)?;
    for entry in WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let rel = path.strip_prefix(src)?;
        let dest = dst.join(rel);
        if path.is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else {
            std::fs::copy(path, &dest)?;
        }
    }
    Ok(())
}

// ── SSR Pass ───────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct PendingSsrJob {
    index: usize,
    url_path: String,
    output_path: PathBuf,
    file_path: PathBuf,
    source_cache_path: PathBuf,
}

/// Run the Puppeteer SSR pass on rendered HTML — side effect: spawns Node.
pub fn run_ssr_pass(
    rendered: &[RenderedPage],
    output_dir: &Path,
    ctx: &BuildContext,
) -> SiteResult<Vec<RenderedPage>> {
    if !ctx.config.build.ssr.enabled {
        return Ok(rendered
            .iter()
            .map(|page| RenderedPage {
                url_path: page.url_path.clone(),
                output_path: page.output_path.clone(),
                html: page.html.clone(),
            })
            .collect());
    }

    let ssr_tags = ctx.components.ssr_component_tags();
    if ssr_tags.is_empty() {
        return Ok(rendered.iter().cloned().collect());
    }

    let ssr_script = find_ssr_script(&ctx.site_root)?;
    let mut results: Vec<Option<RenderedPage>> = (0..rendered.len()).map(|_| None).collect();
    let mut jobs = Vec::new();

    for (index, page) in rendered.iter().enumerate() {
        if !page_needs_ssr(page, &ssr_tags) {
            results[index] = Some(page.clone());
            continue;
        }

        let file_path = output_dir.join(&page.output_path);
        let source_cache_path = ssr_source_cache_path(&ctx.site_root, &page.output_path);
        let source_changed = write_file_if_changed(&source_cache_path, page.html.as_bytes())?;

        if !source_changed && !ssr_needs_rerun(&source_cache_path, &file_path, &ssr_script)? {
            let cached_html = std::fs::read_to_string(&file_path)?;
            results[index] = Some(RenderedPage {
                url_path: page.url_path.clone(),
                output_path: page.output_path.clone(),
                html: cached_html,
            });
            continue;
        }

        write_file_if_changed(&file_path, page.html.as_bytes())?;
        jobs.push(PendingSsrJob {
            index,
            url_path: page.url_path.clone(),
            output_path: page.output_path.clone(),
            file_path,
            source_cache_path,
        });
    }

    if !jobs.is_empty() {
        run_puppeteer_ssr_batch(
            &ctx.site_root,
            output_dir,
            &ctx.config.build.ssr.node_bin,
            &ssr_script,
            ctx.config.build.ssr.timeout_ms,
            ctx.config.build.ssr.concurrency.max(1),
            &jobs,
        )?;

        for job in jobs {
            let html = std::fs::read_to_string(&job.file_path)?;
            results[job.index] = Some(RenderedPage {
                url_path: job.url_path,
                output_path: job.output_path,
                html,
            });
        }
    }

    results
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| SiteError::Ssr("SSR pass did not produce output for every page.".into()))
}

fn page_needs_ssr(page: &RenderedPage, ssr_tags: &[&str]) -> bool {
    ssr_tags.iter().any(|tag| {
        let open_tag = format!("<{}", tag);
        page.html.contains(&open_tag)
    })
}

fn ssr_source_cache_path(site_root: &Path, output_path: &Path) -> PathBuf {
    site_root
        .join(".ferrosite-cache")
        .join("ssr-source")
        .join(output_path)
}

fn ssr_batch_manifest_path(site_root: &Path) -> PathBuf {
    site_root.join(".ferrosite-cache").join("ssr-batch.json")
}

fn metadata_modified(path: &Path) -> SiteResult<Option<SystemTime>> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(Some(metadata.modified()?)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

fn ssr_needs_rerun(source_html: &Path, output_html: &Path, ssr_script: &Path) -> SiteResult<bool> {
    let Some(output_modified) = metadata_modified(output_html)? else {
        return Ok(true);
    };

    let Some(source_modified) = metadata_modified(source_html)? else {
        return Ok(true);
    };

    let Some(script_modified) = metadata_modified(ssr_script)? else {
        return Ok(true);
    };

    Ok(output_modified < source_modified || output_modified < script_modified)
}

fn find_ssr_script(site_root: &Path) -> SiteResult<PathBuf> {
    // Look for ssr/render.mjs relative to site root or cargo manifest
    let candidates = [
        site_root.join("ssr").join("render.mjs"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("ssr")
            .join("render.mjs"),
    ];

    candidates.into_iter().find(|p| p.exists()).ok_or_else(|| {
        SiteError::Ssr("ssr/render.mjs not found. Run 'ferrosite ssr-setup' to install it.".into())
    })
}

fn run_puppeteer_ssr_batch(
    site_root: &Path,
    output_dir: &Path,
    node_bin: &str,
    ssr_script: &Path,
    timeout_ms: u32,
    concurrency: usize,
    jobs: &[PendingSsrJob],
) -> SiteResult<()> {
    let manifest_path = ssr_batch_manifest_path(site_root);
    if let Some(parent) = manifest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let manifest = serde_json::json!({
        "rootDir": output_dir.to_string_lossy(),
        "timeoutMs": timeout_ms,
        "concurrency": concurrency,
        "jobs": jobs.iter().map(|job| serde_json::json!({
            "htmlPath": job.source_cache_path.to_string_lossy(),
            "routePath": job.url_path,
            "outputPath": job.file_path.to_string_lossy(),
        })).collect::<Vec<_>>(),
    });
    std::fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?)?;

    let output = std::process::Command::new(node_bin)
        .arg(ssr_script)
        .arg("--manifest")
        .arg(&manifest_path)
        .output()
        .map_err(|e| SiteError::Ssr(format!("Failed to run node: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SiteError::Ssr(format!(
            "Puppeteer SSR batch failed for {} page(s): {}",
            jobs.len(),
            stderr
        )));
    }

    Ok(())
}

// ── Full Build Pipeline ────────────────────────────────────────────────────────

/// Build report with statistics.
pub struct BuildReport {
    pub pages_built: usize,
    pub articles_processed: usize,
    pub plugins_loaded: usize,
    pub output_dir: PathBuf,
    pub ssr_applied: bool,
}

/// Execute the full site build — orchestrates all pipeline steps.
/// Side effects are isolated to this function and its called steps.
pub fn build_site(site_root: &Path) -> SiteResult<BuildReport> {
    println!("🔧 Loading build context…");
    let mut ctx = BuildContext::load(site_root)?;

    let output_dir = site_root.join(&ctx.config.build.output_dir);
    std::fs::create_dir_all(&output_dir)?;

    println!("📄 Collecting articles…");
    let articles = collect_articles(&ctx)?;
    println!("   {} articles found", articles.len());

    println!("🖼️  Planning media transformations…");
    let media_plan = prepare_media_plan(
        &articles,
        &ctx.config,
        &ctx.site_root,
        &ctx.config.build.content_dir,
        ctx.engine.template_dir(),
        &ctx.config.build.template,
    )?;
    let articles = media_plan.rewritten_articles;
    ctx.config = media_plan.rewritten_config;
    println!("   {} media file(s) queued", media_plan.assets.len());

    println!("🗂️  Building slot maps…");
    let global_slots = build_global_slot_map(&articles)?;

    println!("🏗️  Assembling pages…");
    let page_collection = assemble_pages(&articles, &global_slots, &ctx.config)?;
    println!("   {} pages to render", page_collection.len());

    println!("🎨 Rendering pages…");
    let rendered = render_pages(&page_collection, &ctx)?;

    println!("📦 Copying assets…");
    copy_assets(&ctx, &output_dir)?;

    if !media_plan.assets.is_empty() {
        println!("🖼️  Writing optimized media…");
        write_media_assets(&media_plan.assets, &output_dir)?;
    }

    let ssr_applied = ctx.config.build.ssr.enabled;
    if ssr_applied {
        println!("🖥️  Running SSR pass…");
        let ssr_rendered = run_ssr_pass(&rendered, &output_dir, &ctx)?;
        write_output(&ssr_rendered, &output_dir)?;
    } else {
        println!("💾 Writing output…");
        write_output(&rendered, &output_dir)?;
    }

    println!("✅ Build complete → {}", output_dir.display());

    Ok(BuildReport {
        pages_built: page_collection.len(),
        articles_processed: articles.len(),
        plugins_loaded: ctx.plugins.len(),
        output_dir,
        ssr_applied,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> SiteConfig {
        toml::from_str(
            r#"[site]
title = "Test"
description = "Test site"
base_url = "https://example.com"

[site.author]
name = "Test Author"

[build]
template = "developer"

[deploy]
provider = "cloudflare"

[deploy.cloudflare]
project_name = "test"
account_id = "test"
"#,
        )
        .expect("config should deserialize")
    }

    fn nav_item(title: &str, page_scope: Option<&str>) -> Article {
        let frontmatter = match page_scope {
            Some(scope) => format!(
                r#"---
title = "{}"
slot = "nav-item"
url = "/"
page_scope = "{}"
---
"#,
                title, scope
            ),
            None => format!(
                r#"---
title = "{}"
slot = "nav-item"
url = "/"
---
"#,
                title
            ),
        };

        Article::from_source(
            &frontmatter,
            "nav-home.md",
            "https://example.com",
            &crate::config::RouteConfig::default(),
        )
        .expect("nav item should parse")
    }

    fn blog_post(title: &str, draft: bool) -> Article {
        let frontmatter = format!(
            r#"---
title = "{title}"
slot = "article-body"
date = "2024-01-01"
draft = {draft}
---
Body
"#
        );

        Article::from_source(
            &frontmatter,
            &format!("blog/{}.md", slug::slugify(title)),
            "https://example.com",
            &crate::config::RouteConfig::default(),
        )
        .expect("blog post should parse")
    }

    #[test]
    fn assemble_home_page_does_not_duplicate_global_wildcard_slots() {
        let articles = vec![nav_item("Home", None)];
        let global_slots = build_global_slot_map(&articles).expect("global slots");
        let page = assemble_home_page(&articles, &global_slots, &test_config()).expect("page");

        assert_eq!(page.slots.get(&SlotType::NavItem).len(), 1);
    }

    #[test]
    fn collect_articles_prefers_site_content_over_template_defaults() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let content_dir = root.join("content");
        let template_dir = root.join("templates").join("developer");
        let template_content_dir = root.join("templates").join("developer").join("content");

        std::fs::create_dir_all(&content_dir).expect("site content dir");
        std::fs::create_dir_all(template_dir.join("layouts")).expect("template layouts dir");
        std::fs::create_dir_all(template_dir.join("components")).expect("template components dir");
        std::fs::create_dir_all(&template_content_dir).expect("template content dir");

        std::fs::write(
            root.join("ferrosite.toml"),
            r#"[site]
title = "Test"
description = "Test site"
base_url = "https://example.com"

[site.author]
name = "Test Author"

[build]
template = "developer"
content_dir = "content"

[deploy]
provider = "cloudflare"

[deploy.cloudflare]
project_name = "test"
account_id = "test"
"#,
        )
        .expect("config");

        std::fs::write(
            content_dir.join("nav-home.md"),
            r#"---
title = "Home"
slot = "nav-item"
url = "/site/"
---
"#,
        )
        .expect("site nav");

        std::fs::write(
            template_content_dir.join("nav-home.md"),
            r#"---
title = "Home"
slot = "nav-item"
url = "/template/"
---
"#,
        )
        .expect("template nav");

        let ctx = BuildContext::load(root).expect("build context");
        let articles = collect_articles(&ctx).expect("articles");

        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].frontmatter.url.as_deref(), Some("/site/"));
    }

    #[test]
    fn assemble_pages_keeps_draft_posts_addressable_but_out_of_blog_listing() {
        let articles = vec![
            blog_post("Published Post", false),
            blog_post("Draft Post", true),
        ];
        let global_slots = build_global_slot_map(&articles).expect("global slots");

        let pages = assemble_pages(&articles, &global_slots, &test_config()).expect("pages");

        let blog_page = pages
            .pages
            .iter()
            .find(|page| page.page_type == PageType::Blog)
            .expect("blog page");
        let listed_titles: Vec<_> = blog_page
            .slots
            .get(&SlotType::ArticleCard)
            .iter()
            .map(|article| article.frontmatter.title.as_str())
            .collect();

        assert_eq!(listed_titles, vec!["Published Post"]);

        let post_titles: Vec<_> = pages
            .pages
            .iter()
            .filter(|page| page.page_type == PageType::Post)
            .filter_map(|page| page.primary_article.as_ref())
            .map(|article| article.frontmatter.title.as_str())
            .collect();

        assert!(post_titles.contains(&"Published Post"));
        assert!(post_titles.contains(&"Draft Post"));
    }

    #[test]
    fn write_output_skips_rewriting_identical_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let output_dir = temp.path().join("dist");
        let page = RenderedPage {
            url_path: "/".into(),
            output_path: PathBuf::from("index.html"),
            html: "<html>hello</html>".into(),
        };

        write_output(
            &[RenderedPage {
                url_path: page.url_path.clone(),
                output_path: page.output_path.clone(),
                html: page.html.clone(),
            }],
            &output_dir,
        )
        .expect("first write");

        let file_path = output_dir.join(&page.output_path);
        let first_modified = std::fs::metadata(&file_path)
            .expect("metadata")
            .modified()
            .expect("modified time");

        std::thread::sleep(std::time::Duration::from_millis(1100));

        write_output(&[page], &output_dir).expect("second write");

        let second_modified = std::fs::metadata(&file_path)
            .expect("metadata")
            .modified()
            .expect("modified time");

        assert_eq!(second_modified, first_modified);
    }

    #[test]
    fn ssr_needs_rerun_only_when_source_or_script_is_newer() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let source = root
            .join(".ferrosite-cache")
            .join("ssr-source")
            .join("index.html");
        let output = root.join("dist").join("index.html");
        let script = root.join("ssr").join("render.mjs");

        write_file_if_changed(&source, b"<html>source</html>").expect("write source");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        write_file_if_changed(&output, b"<html>ssr</html>").expect("write output");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        write_file_if_changed(&script, b"console.log('ssr');").expect("write script");

        assert!(ssr_needs_rerun(&source, &output, &script).expect("freshness"));

        std::thread::sleep(std::time::Duration::from_millis(1100));
        write_file_if_changed(&output, b"<html>ssr newer</html>").expect("refresh output");

        assert!(!ssr_needs_rerun(&source, &output, &script).expect("freshness"));

        std::thread::sleep(std::time::Duration::from_millis(1100));
        write_file_if_changed(&source, b"<html>source changed</html>").expect("rewrite source");

        assert!(ssr_needs_rerun(&source, &output, &script).expect("freshness"));
    }

    #[test]
    fn page_needs_ssr_matches_known_custom_elements() {
        let page = RenderedPage {
            url_path: "/projects/".into(),
            output_path: PathBuf::from("projects/index.html"),
            html: r#"<main><dev-project-grid projects="[]"></dev-project-grid></main>"#.into(),
        };

        assert!(page_needs_ssr(
            &page,
            &["dev-project-card", "dev-project-grid"]
        ));
    }

    #[test]
    fn page_needs_ssr_skips_plain_html_pages() {
        let page = RenderedPage {
            url_path: "/about/".into(),
            output_path: PathBuf::from("about/index.html"),
            html: "<main><h1>About</h1><p>Plain HTML.</p></main>".into(),
        };

        assert!(!page_needs_ssr(
            &page,
            &["dev-project-card", "dev-project-grid"]
        ));
    }
}
