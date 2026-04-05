use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufWriter;
use std::path::{Component, Path, PathBuf};
#[cfg(test)]
use std::time::SystemTime;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilterType, PngEncoder};
use image::imageops::FilterType;
use image::{ImageEncoder, ImageReader};
use pulldown_cmark::{Event, Parser, Tag};

use crate::config::SiteConfig;
use crate::content::article::Article;
use crate::error::{SiteError, SiteResult};
use crate::url::{decode_url_path, encode_url_path};

const STATIC_MEDIA_DIR: &str = "static/media";
const MAX_RASTER_WIDTH: u32 = 1600;
const JPEG_QUALITY: u8 = 82;

#[derive(Debug, Clone)]
pub struct MediaPlan {
    pub assets: Vec<MediaAsset>,
    pub rewritten_articles: Vec<Article>,
    pub rewritten_config: SiteConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaAsset {
    pub source_path: PathBuf,
    pub output_rel_path: PathBuf,
    pub public_url: String,
    pub transform: MediaTransform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaTransform {
    OptimizeRaster,
    CopyOriginal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MediaRequest {
    owner: MediaOwner,
    original_value: String,
    media_url: String,
    format: MediaValueFormat,
}

#[derive(Debug, Clone)]
struct ResolvedMedia {
    owner: MediaOwner,
    original_value: String,
    replacement_value: String,
    html_url_replacement: Option<(String, String)>,
    asset: MediaAsset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MediaOwner {
    Article(String),
    SiteConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MediaValueFormat {
    PlainUrl,
    CssUrl { quote: Option<char> },
}

#[derive(Debug, Clone)]
struct MediaCandidate {
    source_path: PathBuf,
    base_root: PathBuf,
    namespace: MediaNamespace,
}

#[derive(Debug, Clone, Copy)]
enum MediaNamespace {
    SiteContent,
    SiteRoot,
    TemplateContent,
    TemplateRoot,
}

impl MediaNamespace {
    fn append_to(self, path: &mut PathBuf, template_name: &str) {
        match self {
            Self::SiteContent => {
                path.push("site");
                path.push("content");
            }
            Self::SiteRoot => {
                path.push("site");
                path.push("root");
            }
            Self::TemplateContent => {
                path.push("template");
                path.push(template_name);
                path.push("content");
            }
            Self::TemplateRoot => {
                path.push("template");
                path.push(template_name);
                path.push("root");
            }
        }
    }
}

pub fn prepare_media_plan(
    articles: &[Article],
    config: &SiteConfig,
    site_root: &Path,
    content_dir: &str,
    template_dir: &Path,
    template_name: &str,
) -> SiteResult<MediaPlan> {
    let requests = collect_media_requests(articles, config);
    if requests.is_empty() {
        return Ok(MediaPlan {
            assets: Vec::new(),
            rewritten_articles: articles.to_vec(),
            rewritten_config: config.clone(),
        });
    }

    let site_content_root = site_root.join(content_dir);
    let template_content_root = template_dir.join("content");

    let resolved = requests
        .iter()
        .map(|request| {
            resolve_media_request(
                request,
                site_root,
                &site_content_root,
                template_dir,
                &template_content_root,
                template_name,
            )
        })
        .collect::<SiteResult<Vec<_>>>()?;

    let mut assets = Vec::new();
    let mut seen_assets = HashSet::new();
    let mut article_html_replacements: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut article_field_replacements: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut config_replacements: HashMap<String, String> = HashMap::new();

    for item in resolved {
        match &item.owner {
            MediaOwner::Article(source_path) => {
                article_field_replacements
                    .entry(source_path.clone())
                    .or_default()
                    .insert(item.original_value.clone(), item.replacement_value.clone());

                if let Some((from, to)) = &item.html_url_replacement {
                    article_html_replacements
                        .entry(source_path.clone())
                        .or_default()
                        .insert(from.clone(), to.clone());
                }
            }
            MediaOwner::SiteConfig => {
                config_replacements
                    .insert(item.original_value.clone(), item.replacement_value.clone());
            }
        }

        let asset_key = (
            item.asset.source_path.clone(),
            item.asset.output_rel_path.clone(),
            item.asset.transform,
        );
        if seen_assets.insert(asset_key) {
            assets.push(item.asset);
        }
    }

    assets.sort_by(|left, right| left.output_rel_path.cmp(&right.output_rel_path));

    let rewritten_articles = articles
        .iter()
        .map(|article| {
            let html_replacements = article_html_replacements.get(&article.source_path);
            let field_replacements = article_field_replacements.get(&article.source_path);

            match (html_replacements, field_replacements) {
                (None, None) => article.clone(),
                _ => apply_media_rewrites(article, html_replacements, field_replacements),
            }
        })
        .collect();

    let rewritten_config = apply_site_config_media_rewrites(config, &config_replacements);

    Ok(MediaPlan {
        assets,
        rewritten_articles,
        rewritten_config,
    })
}

fn collect_media_requests(articles: &[Article], config: &SiteConfig) -> Vec<MediaRequest> {
    let mut requests = Vec::new();

    for article in articles {
        let owner = MediaOwner::Article(article.source_path.clone());

        requests.extend(
            extract_markdown_image_sources(&article.markdown_body)
                .into_iter()
                .map(|original_url| MediaRequest {
                    owner: owner.clone(),
                    media_url: original_url.clone(),
                    original_value: original_url,
                    format: MediaValueFormat::PlainUrl,
                }),
        );

        requests.extend(
            [
                media_request_from_plain_value(
                    owner.clone(),
                    article.frontmatter.cover_image.as_deref(),
                ),
                media_request_from_plain_value(
                    owner.clone(),
                    article.frontmatter.og_image.as_deref(),
                ),
                media_request_from_css_or_plain_value(
                    owner.clone(),
                    article.frontmatter.background.as_deref(),
                ),
            ]
            .into_iter()
            .flatten(),
        );
    }

    requests.extend(
        [
            media_request_from_plain_value(
                MediaOwner::SiteConfig,
                config.site.author.avatar.as_deref(),
            ),
            media_request_from_plain_value(MediaOwner::SiteConfig, config.site.favicon.as_deref()),
        ]
        .into_iter()
        .flatten(),
    );

    requests
}

pub fn extract_markdown_image_sources(markdown: &str) -> Vec<String> {
    Parser::new(markdown)
        .filter_map(|event| match event {
            Event::Start(Tag::Image(_, url, _)) if should_process_media_url(url.as_ref()) => {
                Some(url.to_string())
            }
            _ => None,
        })
        .collect()
}

pub fn apply_media_rewrites(
    article: &Article,
    html_replacements: Option<&HashMap<String, String>>,
    field_replacements: Option<&HashMap<String, String>>,
) -> Article {
    let mut rewritten = article.clone();

    if let Some(replacements) = html_replacements {
        rewritten.html_body = rewrite_html_image_sources(&article.html_body, replacements);
    }

    if let Some(replacements) = field_replacements {
        rewrite_optional_media_value(&mut rewritten.frontmatter.cover_image, replacements);
        rewrite_optional_media_value(&mut rewritten.frontmatter.og_image, replacements);
        rewrite_optional_media_value(&mut rewritten.frontmatter.background, replacements);
    }

    rewritten
}

pub fn apply_site_config_media_rewrites(
    config: &SiteConfig,
    replacements: &HashMap<String, String>,
) -> SiteConfig {
    let mut rewritten = config.clone();
    rewrite_optional_media_value(&mut rewritten.site.author.avatar, replacements);
    rewrite_optional_media_value(&mut rewritten.site.favicon, replacements);
    rewritten
}

pub fn rewrite_html_image_sources(html: &str, replacements: &HashMap<String, String>) -> String {
    replacements
        .iter()
        .fold(html.to_string(), |acc, (from, to)| {
            let escaped_from = escape_html_attribute(from);
            let escaped_to = escape_html_attribute(to);
            acc.replace(
                &format!("src=\"{}\"", escaped_from),
                &format!("src=\"{}\"", escaped_to),
            )
            .replace(
                &format!("src='{}'", escaped_from),
                &format!("src='{}'", escaped_to),
            )
        })
}

pub fn write_media_assets(assets: &[MediaAsset], output_dir: &Path) -> SiteResult<Vec<PathBuf>> {
    assets
        .iter()
        .map(|asset| write_media_asset(asset, output_dir))
        .collect()
}

fn destination_is_up_to_date(source: &Path, destination: &Path) -> SiteResult<bool> {
    let destination_metadata = match std::fs::metadata(destination) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err.into()),
    };

    let source_modified = std::fs::metadata(source)?.modified()?;
    let destination_modified = destination_metadata.modified()?;

    Ok(destination_modified >= source_modified)
}

fn resolve_media_request(
    request: &MediaRequest,
    site_root: &Path,
    site_content_root: &Path,
    template_dir: &Path,
    template_content_root: &Path,
    template_name: &str,
) -> SiteResult<ResolvedMedia> {
    let (media_path, suffix) = split_url_suffix(&request.media_url);
    let decoded_media_path = decode_url_path(media_path).ok_or_else(|| SiteError::Asset {
        asset: request.media_url.clone(),
        message: format!(
            "Referenced image '{}' contains invalid URL encoding.",
            request.media_url
        ),
    })?;
    let article_dir = match &request.owner {
        MediaOwner::Article(source_path) => Path::new(source_path)
            .parent()
            .unwrap_or_else(|| Path::new("")),
        MediaOwner::SiteConfig => Path::new(""),
    };

    let candidates = build_candidates(
        &decoded_media_path,
        article_dir,
        site_root,
        site_content_root,
        template_dir,
        template_content_root,
    );

    let candidate = candidates
        .into_iter()
        .find(|candidate| candidate.source_path.exists() && candidate.source_path.is_file())
        .ok_or_else(|| SiteError::Asset {
            asset: request.media_url.clone(),
            message: format!(
                "Referenced image '{}' could not be resolved.",
                request.media_url
            ),
        })?;

    let normalized_source = normalize_path(&candidate.source_path);
    let normalized_base = normalize_path(&candidate.base_root);

    if !normalized_source.starts_with(&normalized_base) {
        return Err(SiteError::Asset {
            asset: request.media_url.clone(),
            message: format!(
                "Resolved image path '{}' escapes the allowed content roots.",
                normalized_source.display()
            ),
        });
    }

    let relative_source = normalized_source.strip_prefix(&normalized_base)?;
    let mut output_rel_path = PathBuf::from(STATIC_MEDIA_DIR);
    candidate
        .namespace
        .append_to(&mut output_rel_path, template_name);
    output_rel_path.push(relative_source);

    let public_url = format!("/{}{}", to_url_path(&output_rel_path), suffix);
    let html_source_url = format!("{}{}", encode_url_path(&decoded_media_path), suffix);
    let replacement_value = render_media_value(&public_url, request.format);
    let asset = MediaAsset {
        source_path: candidate.source_path,
        output_rel_path,
        public_url: public_url.clone(),
        transform: select_media_transform(relative_source),
    };

    Ok(ResolvedMedia {
        owner: request.owner.clone(),
        original_value: request.original_value.clone(),
        replacement_value,
        html_url_replacement: matches!(request.format, MediaValueFormat::PlainUrl)
            .then(|| (html_source_url, public_url.clone())),
        asset,
    })
}

fn build_candidates(
    media_path: &str,
    article_dir: &Path,
    site_root: &Path,
    site_content_root: &Path,
    template_dir: &Path,
    template_content_root: &Path,
) -> Vec<MediaCandidate> {
    let relative_path = Path::new(media_path.trim_start_matches('/'));

    if media_path.starts_with('/') {
        vec![
            MediaCandidate {
                source_path: normalize_path(&site_root.join(relative_path)),
                base_root: normalize_path(site_root),
                namespace: MediaNamespace::SiteRoot,
            },
            MediaCandidate {
                source_path: normalize_path(&template_dir.join(relative_path)),
                base_root: normalize_path(template_dir),
                namespace: MediaNamespace::TemplateRoot,
            },
        ]
    } else {
        vec![
            MediaCandidate {
                source_path: normalize_path(
                    &site_content_root.join(article_dir).join(relative_path),
                ),
                base_root: normalize_path(site_content_root),
                namespace: MediaNamespace::SiteContent,
            },
            MediaCandidate {
                source_path: normalize_path(
                    &template_content_root.join(article_dir).join(relative_path),
                ),
                base_root: normalize_path(template_content_root),
                namespace: MediaNamespace::TemplateContent,
            },
            MediaCandidate {
                source_path: normalize_path(&site_root.join(relative_path)),
                base_root: normalize_path(site_root),
                namespace: MediaNamespace::SiteRoot,
            },
            MediaCandidate {
                source_path: normalize_path(&template_dir.join(relative_path)),
                base_root: normalize_path(template_dir),
                namespace: MediaNamespace::TemplateRoot,
            },
        ]
    }
}

fn write_media_asset(asset: &MediaAsset, output_dir: &Path) -> SiteResult<PathBuf> {
    let destination = output_dir.join(&asset.output_rel_path);
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if destination_is_up_to_date(&asset.source_path, &destination)? {
        return Ok(destination);
    }

    match asset.transform {
        MediaTransform::OptimizeRaster => optimize_raster_image(&asset.source_path, &destination)?,
        MediaTransform::CopyOriginal => {
            std::fs::copy(&asset.source_path, &destination)?;
        }
    }

    Ok(destination)
}

#[cfg(test)]
fn modification_time(path: &Path) -> SystemTime {
    std::fs::metadata(path)
        .expect("metadata")
        .modified()
        .expect("modified time")
}

fn optimize_raster_image(source_path: &Path, destination: &Path) -> SiteResult<()> {
    let reader = ImageReader::open(source_path).map_err(|source| SiteError::Asset {
        asset: source_path.display().to_string(),
        message: source.to_string(),
    })?;
    let image = reader.decode().map_err(|source| SiteError::Asset {
        asset: source_path.display().to_string(),
        message: source.to_string(),
    })?;

    let optimized = if image.width() > MAX_RASTER_WIDTH {
        image.resize(MAX_RASTER_WIDTH, u32::MAX, FilterType::Lanczos3)
    } else {
        image
    };

    match extension_lowercase(destination).as_deref() {
        Some("jpg") | Some("jpeg") => {
            let file = File::create(destination)?;
            let writer = BufWriter::new(file);
            let mut encoder = JpegEncoder::new_with_quality(writer, JPEG_QUALITY);
            encoder
                .encode_image(&optimized)
                .map_err(|source| SiteError::Asset {
                    asset: source_path.display().to_string(),
                    message: source.to_string(),
                })?;
        }
        Some("png") => {
            let rgba = optimized.to_rgba8();
            let file = File::create(destination)?;
            let writer = BufWriter::new(file);
            let encoder = PngEncoder::new_with_quality(
                writer,
                CompressionType::Best,
                PngFilterType::Adaptive,
            );
            encoder
                .write_image(
                    rgba.as_raw(),
                    rgba.width(),
                    rgba.height(),
                    image::ColorType::Rgba8.into(),
                )
                .map_err(|source| SiteError::Asset {
                    asset: source_path.display().to_string(),
                    message: source.to_string(),
                })?;
        }
        _ => {
            std::fs::copy(source_path, destination)?;
        }
    }

    Ok(())
}

fn select_media_transform(path: &Path) -> MediaTransform {
    match extension_lowercase(path).as_deref() {
        Some("jpg") | Some("jpeg") | Some("png") => MediaTransform::OptimizeRaster,
        _ => MediaTransform::CopyOriginal,
    }
}

fn extension_lowercase(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

fn should_process_media_url(url: &str) -> bool {
    let trimmed = url.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with("http://")
        && !trimmed.starts_with("https://")
        && !trimmed.starts_with("//")
        && !trimmed.starts_with("data:")
        && !trimmed.starts_with("mailto:")
        && !trimmed.starts_with("tel:")
        && !trimmed.starts_with('#')
}

fn media_request_from_plain_value(owner: MediaOwner, value: Option<&str>) -> Option<MediaRequest> {
    let value = value?.trim();
    should_process_media_url(value).then(|| MediaRequest {
        owner,
        original_value: value.to_string(),
        media_url: value.to_string(),
        format: MediaValueFormat::PlainUrl,
    })
}

fn media_request_from_css_or_plain_value(
    owner: MediaOwner,
    value: Option<&str>,
) -> Option<MediaRequest> {
    let value = value?.trim();

    if let Some((media_url, quote)) = parse_css_url_value(value) {
        return Some(MediaRequest {
            owner,
            original_value: value.to_string(),
            media_url,
            format: MediaValueFormat::CssUrl { quote },
        });
    }

    media_request_from_plain_value(owner, Some(value))
}

fn parse_css_url_value(value: &str) -> Option<(String, Option<char>)> {
    let trimmed = value.trim();
    if !(trimmed.starts_with("url(") && trimmed.ends_with(')')) {
        return None;
    }

    let inner = trimmed[4..trimmed.len() - 1].trim();
    let (media_url, quote) = match (inner.chars().next(), inner.chars().last()) {
        (Some('\''), Some('\'')) if inner.len() >= 2 => (&inner[1..inner.len() - 1], Some('\'')),
        (Some('"'), Some('"')) if inner.len() >= 2 => (&inner[1..inner.len() - 1], Some('"')),
        _ => (inner, None),
    };

    should_process_media_url(media_url).then(|| (media_url.to_string(), quote))
}

fn render_media_value(public_url: &str, format: MediaValueFormat) -> String {
    match format {
        MediaValueFormat::PlainUrl => public_url.to_string(),
        MediaValueFormat::CssUrl { quote: Some(quote) } => {
            format!("url({quote}{public_url}{quote})")
        }
        MediaValueFormat::CssUrl { quote: None } => format!("url({public_url})"),
    }
}

fn rewrite_optional_media_value(
    value: &mut Option<String>,
    replacements: &HashMap<String, String>,
) {
    if let Some(current) = value.as_mut() {
        if let Some(rewritten) = replacements.get(current.as_str()) {
            *current = rewritten.clone();
        }
    }
}

fn split_url_suffix(url: &str) -> (&str, &str) {
    let suffix_start = url.find(['?', '#']).unwrap_or(url.len());
    (&url[..suffix_start], &url[suffix_start..])
}

fn escape_html_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push("..");
                }
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }

    normalized
}

fn to_url_path(path: &Path) -> String {
    encode_url_path(&path.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_site_config;
    use crate::content::frontmatter::Frontmatter;

    #[test]
    fn extracts_only_local_markdown_images() {
        let markdown = r#"
![Hero](./hero.png)
![Absolute](/assets/logo.png)
![Remote](https://example.com/hero.png)
[Link](./hero.png)
"#;

        let found = extract_markdown_image_sources(markdown);

        assert_eq!(found, vec!["./hero.png", "/assets/logo.png"]);
    }

    #[test]
    fn rewrite_html_image_sources_only_updates_image_src_attributes() {
        let mut replacements = HashMap::new();
        replacements.insert(
            "./hero.png".to_string(),
            "/static/media/site/content/blog/hero.png".to_string(),
        );

        let html =
            r#"<p><img src="./hero.png" alt="Hero"></p><p><a href="./hero.png">download</a></p>"#;
        let rewritten = rewrite_html_image_sources(html, &replacements);

        assert!(rewritten.contains("src=\"/static/media/site/content/blog/hero.png\""));
        assert!(rewritten.contains("href=\"./hero.png\""));
    }

    #[test]
    fn rewrite_html_image_sources_matches_encoded_markdown_urls() {
        let mut replacements = HashMap::new();
        replacements.insert(
            "./hero%20image.png".to_string(),
            "/static/media/site/content/blog/hero%20image.png".to_string(),
        );

        let html = r#"<p><img src="./hero%20image.png" alt="Hero"></p>"#;
        let rewritten = rewrite_html_image_sources(html, &replacements);

        assert!(rewritten.contains(r#"src="/static/media/site/content/blog/hero%20image.png""#));
    }

    #[test]
    fn rewrites_site_config_media_fields() {
        let config = load_site_config_from_raw(
            r#"
[site]
title = "Example"
description = "Site"
base_url = "https://example.com"
favicon = "/assets/favicon.png"

[site.author]
name = "Author"
avatar = "/assets/avatar.png"

[build]
template = "developer"

[deploy]
provider = "cloudflare"

[deploy.cloudflare]
project_name = "example"
account_id = "abc123"
"#,
        );

        let replacements = HashMap::from([
            (
                "/assets/avatar.png".to_string(),
                "/static/media/site/root/assets/avatar.png".to_string(),
            ),
            (
                "/assets/favicon.png".to_string(),
                "/static/media/site/root/assets/favicon.png".to_string(),
            ),
        ]);

        let rewritten = apply_site_config_media_rewrites(&config, &replacements);

        assert_eq!(
            rewritten.site.author.avatar.as_deref(),
            Some("/static/media/site/root/assets/avatar.png")
        );
        assert_eq!(
            rewritten.site.favicon.as_deref(),
            Some("/static/media/site/root/assets/favicon.png")
        );
    }

    #[test]
    fn prepare_media_plan_rewrites_articles_config_and_writes_assets() {
        let temp = tempfile::tempdir().expect("tempdir");
        let site_root = temp.path();
        let content_dir = site_root.join("content").join("blog");
        let template_dir = site_root.join("templates").join("developer");
        let assets_dir = site_root.join("assets");

        std::fs::create_dir_all(&content_dir).expect("content dir");
        std::fs::create_dir_all(template_dir.join("content")).expect("template content dir");
        std::fs::create_dir_all(&assets_dir).expect("assets dir");

        image::DynamicImage::new_rgba8(32, 32)
            .save(content_dir.join("hero.png"))
            .expect("write png");
        image::DynamicImage::new_rgba8(32, 32)
            .save(assets_dir.join("avatar.png"))
            .expect("write avatar");
        image::DynamicImage::new_rgba8(32, 32)
            .save(assets_dir.join("favicon.png"))
            .expect("write favicon");

        let config = load_site_config_from_raw(
            r#"
[site]
title = "Example"
description = "Site"
base_url = "https://example.com"
favicon = "/assets/favicon.png"

[site.author]
name = "Author"
avatar = "/assets/avatar.png"

[build]
template = "developer"

[deploy]
provider = "cloudflare"

[deploy.cloudflare]
project_name = "example"
account_id = "abc123"
"#,
        );

        let article = Article::from_raw_document(
            Frontmatter {
                title: "Post".into(),
                slot: "article-body".into(),
                cover_image: Some("./hero.png".into()),
                og_image: Some("./hero.png".into()),
                background: Some("url('./hero.png')".into()),
                ..Default::default()
            },
            "![Hero](./hero.png)".into(),
            "blog/post.md",
            "https://example.com",
            &crate::config::RouteConfig::default(),
        )
        .expect("article");

        let plan = prepare_media_plan(
            &[article],
            &config,
            site_root,
            "content",
            &template_dir,
            "developer",
        )
        .expect("media plan");

        assert_eq!(plan.assets.len(), 3);
        let asset_paths: Vec<_> = plan
            .assets
            .iter()
            .map(|asset| asset.output_rel_path.clone())
            .collect();
        assert!(asset_paths.contains(&PathBuf::from("static/media/site/root/assets/avatar.png")));
        assert!(asset_paths.contains(&PathBuf::from("static/media/site/root/assets/favicon.png")));
        assert!(asset_paths.contains(&PathBuf::from("static/media/site/content/blog/hero.png")));
        assert!(plan.rewritten_articles[0]
            .html_body
            .contains("/static/media/site/content/blog/hero.png"));
        assert_eq!(
            plan.rewritten_articles[0]
                .frontmatter
                .cover_image
                .as_deref(),
            Some("/static/media/site/content/blog/hero.png")
        );
        assert_eq!(
            plan.rewritten_articles[0].frontmatter.og_image.as_deref(),
            Some("/static/media/site/content/blog/hero.png")
        );
        assert_eq!(
            plan.rewritten_articles[0].frontmatter.background.as_deref(),
            Some("url('/static/media/site/content/blog/hero.png')")
        );
        assert_eq!(
            plan.rewritten_config.site.author.avatar.as_deref(),
            Some("/static/media/site/root/assets/avatar.png")
        );
        assert_eq!(
            plan.rewritten_config.site.favicon.as_deref(),
            Some("/static/media/site/root/assets/favicon.png")
        );

        let output_dir = site_root.join("dist");
        write_media_assets(&plan.assets, &output_dir).expect("write media assets");

        assert!(output_dir
            .join("static/media/site/content/blog/hero.png")
            .exists());
        assert!(output_dir
            .join("static/media/site/root/assets/avatar.png")
            .exists());
        assert!(output_dir
            .join("static/media/site/root/assets/favicon.png")
            .exists());
    }

    #[test]
    fn prepare_media_plan_encodes_public_urls_for_spaced_media_paths() {
        let temp = tempfile::tempdir().expect("tempdir");
        let site_root = temp.path();
        let content_dir = site_root.join("content").join("blog");
        let template_dir = site_root.join("templates").join("developer");

        std::fs::create_dir_all(&content_dir).expect("content dir");
        std::fs::create_dir_all(template_dir.join("content")).expect("template content dir");

        image::DynamicImage::new_rgba8(32, 32)
            .save(content_dir.join("hero image.png"))
            .expect("write spaced png");

        let config = load_site_config_from_raw(
            r#"
[site]
title = "Example"
description = "Site"
base_url = "https://example.com"

[site.author]
name = "Author"

[build]
template = "developer"

[deploy]
provider = "cloudflare"

[deploy.cloudflare]
project_name = "example"
account_id = "abc123"
"#,
        );

        let article = Article::from_raw_document(
            Frontmatter {
                title: "Post".into(),
                slot: "article-body".into(),
                cover_image: Some("./hero image.png".into()),
                ..Default::default()
            },
            "![Hero](<./hero image.png>)".into(),
            "blog/post.md",
            "https://example.com",
            &crate::config::RouteConfig::default(),
        )
        .expect("article");

        let plan = prepare_media_plan(
            &[article],
            &config,
            site_root,
            "content",
            &template_dir,
            "developer",
        )
        .expect("media plan");

        assert!(plan.rewritten_articles[0]
            .html_body
            .contains("/static/media/site/content/blog/hero%20image.png"));
        assert_eq!(
            plan.rewritten_articles[0]
                .frontmatter
                .cover_image
                .as_deref(),
            Some("/static/media/site/content/blog/hero%20image.png")
        );
        assert_eq!(
            plan.assets[0].public_url,
            "/static/media/site/content/blog/hero%20image.png"
        );
    }

    #[test]
    fn write_media_assets_skips_outputs_that_are_already_fresh() {
        let temp = tempfile::tempdir().expect("tempdir");
        let site_root = temp.path();
        let source_path = site_root.join("content").join("hero.png");
        let output_dir = site_root.join("dist");

        std::fs::create_dir_all(source_path.parent().expect("parent")).expect("source dir");

        image::DynamicImage::new_rgba8(32, 32)
            .save(&source_path)
            .expect("write source png");

        let asset = MediaAsset {
            source_path: source_path.clone(),
            output_rel_path: PathBuf::from("static/media/site/content/hero.png"),
            public_url: "/static/media/site/content/hero.png".into(),
            transform: MediaTransform::OptimizeRaster,
        };

        write_media_assets(&[asset.clone()], &output_dir).expect("initial write");

        let destination = output_dir.join(&asset.output_rel_path);
        let first_modified = modification_time(&destination);

        std::thread::sleep(std::time::Duration::from_millis(1100));
        write_media_assets(&[asset.clone()], &output_dir).expect("second write");
        let second_modified = modification_time(&destination);
        assert_eq!(second_modified, first_modified);

        std::thread::sleep(std::time::Duration::from_millis(1100));
        image::DynamicImage::new_rgba8(48, 48)
            .save(&source_path)
            .expect("rewrite source png");

        write_media_assets(&[asset], &output_dir).expect("refresh write");
        let refreshed_modified = modification_time(&destination);
        assert!(refreshed_modified > second_modified);
    }

    fn load_site_config_from_raw(raw: &str) -> SiteConfig {
        let temp = tempfile::tempdir().expect("tempdir");
        let config_path = temp.path().join("ferrosite.toml");
        std::fs::write(&config_path, raw).expect("config");
        load_site_config(&config_path).expect("site config")
    }
}
