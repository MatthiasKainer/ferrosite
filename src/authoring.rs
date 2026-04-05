use std::path::{Path, PathBuf};

use slug::slugify;
use walkdir::WalkDir;

use crate::config::load_site_config_for_root;
use crate::content::frontmatter::split_frontmatter;
use crate::content::slot::SlotType;
use crate::error::{io_with_path, SiteError, SiteResult};

#[derive(Debug, Clone)]
pub struct NewArticleRequest {
    pub title: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub date: String,
    pub tags: Vec<String>,
    pub categories: Vec<String>,
    pub featured: bool,
    pub draft: bool,
    pub path: Option<PathBuf>,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct NewProjectRequest {
    pub title: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub tech_stack: Vec<String>,
    pub repo_url: Option<String>,
    pub live_url: Option<String>,
    pub path: Option<PathBuf>,
    pub body: String,
}

#[derive(Debug, Clone)]
pub struct NewPageRequest {
    pub title: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub slot: String,
    pub page_scope: String,
    pub url: Option<String>,
    pub order: i32,
    pub weight: i32,
    pub path: Option<PathBuf>,
    pub body: String,
    pub nav: Option<NewNavRequest>,
}

#[derive(Debug, Clone)]
pub struct NewNavRequest {
    pub title: String,
    pub url: String,
    pub order: i32,
    pub weight: i32,
    pub icon: Option<String>,
    pub external: bool,
    pub target_page: Option<String>,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct EditContentRequest {
    pub target: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub slug: Option<String>,
    pub slot: Option<String>,
    pub page_scope: Option<String>,
    pub order: Option<i32>,
    pub weight: Option<i32>,
    pub url: Option<String>,
    pub date: Option<String>,
    pub author: Option<String>,
    pub tags: Option<Vec<String>>,
    pub categories: Option<Vec<String>>,
    pub tech_stack: Option<Vec<String>>,
    pub repo_url: Option<String>,
    pub live_url: Option<String>,
    pub status: Option<String>,
    pub icon: Option<String>,
    pub target_page: Option<String>,
    pub body: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AssignSlotRequest {
    pub target: String,
    pub slot: String,
    pub page_scope: Option<String>,
    pub order: Option<i32>,
    pub weight: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct WriteOutcome {
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PageWriteOutcome {
    pub page_path: PathBuf,
    pub nav_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ContentDocument {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub frontmatter: toml::map::Map<String, toml::Value>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReorderEntry {
    pub path: PathBuf,
    pub relative_path: PathBuf,
    pub title: String,
    pub slot: String,
    pub page_scope: String,
    pub order: i32,
    pub weight: i32,
}

pub fn create_article(root: &Path, request: &NewArticleRequest) -> SiteResult<WriteOutcome> {
    let slug = slug_or_default(request.slug.as_deref(), &request.title);
    let path = defaulted_output_path(root, request.path.as_ref(), &format!("blog/{}.md", slug))?;

    let mut frontmatter = toml::map::Map::new();
    frontmatter.insert("title".into(), toml::Value::String(request.title.clone()));
    frontmatter.insert("slug".into(), toml::Value::String(slug));
    frontmatter.insert("slot".into(), toml::Value::String("article-body".into()));
    frontmatter.insert("page_scope".into(), toml::Value::String("blog".into()));
    frontmatter.insert("date".into(), toml::Value::String(request.date.clone()));
    set_optional_string(&mut frontmatter, "description", request.description.clone());
    set_optional_string(&mut frontmatter, "author", request.author.clone());
    set_string_array(&mut frontmatter, "tags", &request.tags);
    set_string_array(&mut frontmatter, "categories", &request.categories);
    if request.featured {
        frontmatter.insert("featured".into(), toml::Value::Boolean(true));
    }
    if request.draft {
        frontmatter.insert("draft".into(), toml::Value::Boolean(true));
    }

    write_new_document(&path, frontmatter, &request.body)?;
    Ok(WriteOutcome { path })
}

pub fn create_project(root: &Path, request: &NewProjectRequest) -> SiteResult<WriteOutcome> {
    let slug = slug_or_default(request.slug.as_deref(), &request.title);
    let path = defaulted_output_path(
        root,
        request.path.as_ref(),
        &format!("projects/{}.md", slug),
    )?;

    let mut frontmatter = toml::map::Map::new();
    frontmatter.insert("title".into(), toml::Value::String(request.title.clone()));
    frontmatter.insert("slug".into(), toml::Value::String(slug));
    frontmatter.insert("slot".into(), toml::Value::String("project-body".into()));
    frontmatter.insert("page_scope".into(), toml::Value::String("projects".into()));
    set_optional_string(&mut frontmatter, "description", request.description.clone());
    set_optional_string(&mut frontmatter, "status", request.status.clone());
    set_optional_string(&mut frontmatter, "repo_url", request.repo_url.clone());
    set_optional_string(&mut frontmatter, "live_url", request.live_url.clone());
    set_string_array(&mut frontmatter, "tech_stack", &request.tech_stack);

    write_new_document(&path, frontmatter, &request.body)?;
    Ok(WriteOutcome { path })
}

pub fn create_page(root: &Path, request: &NewPageRequest) -> SiteResult<PageWriteOutcome> {
    validate_slot_scope(&request.slot, &request.page_scope)?;

    let slug = slug_or_default(request.slug.as_deref(), &request.title);
    let fallback = default_page_path(&slug, &request.slot, &request.page_scope);
    let page_path = defaulted_output_path(root, request.path.as_ref(), &fallback)?;

    let mut frontmatter = toml::map::Map::new();
    frontmatter.insert("title".into(), toml::Value::String(request.title.clone()));
    frontmatter.insert("slot".into(), toml::Value::String(request.slot.clone()));
    frontmatter.insert(
        "page_scope".into(),
        toml::Value::String(request.page_scope.clone()),
    );
    frontmatter.insert("order".into(), toml::Value::Integer(request.order as i64));
    frontmatter.insert("weight".into(), toml::Value::Integer(request.weight as i64));
    if should_write_slug(&request.slot) {
        frontmatter.insert("slug".into(), toml::Value::String(slug.clone()));
    }
    set_optional_string(&mut frontmatter, "description", request.description.clone());
    set_optional_string(&mut frontmatter, "url", request.url.clone());

    write_new_document(&page_path, frontmatter, &request.body)?;

    let nav_path = if let Some(nav_request) = &request.nav {
        Some(create_nav(root, nav_request)?.path)
    } else {
        None
    };

    Ok(PageWriteOutcome {
        page_path,
        nav_path,
    })
}

pub fn create_nav(root: &Path, request: &NewNavRequest) -> SiteResult<WriteOutcome> {
    let slug = slug_or_default(None, &request.title);
    let path = defaulted_output_path(root, request.path.as_ref(), &format!("nav-{}.md", slug))?;

    let mut frontmatter = toml::map::Map::new();
    frontmatter.insert("title".into(), toml::Value::String(request.title.clone()));
    frontmatter.insert("slot".into(), toml::Value::String("nav-item".into()));
    frontmatter.insert("page_scope".into(), toml::Value::String("*".into()));
    frontmatter.insert("order".into(), toml::Value::Integer(request.order as i64));
    frontmatter.insert("weight".into(), toml::Value::Integer(request.weight as i64));
    frontmatter.insert("url".into(), toml::Value::String(request.url.clone()));
    if request.external {
        frontmatter.insert("external".into(), toml::Value::Boolean(true));
    }
    set_optional_string(&mut frontmatter, "icon", request.icon.clone());
    set_optional_string(&mut frontmatter, "target_page", request.target_page.clone());

    write_new_document(&path, frontmatter, "")?;
    Ok(WriteOutcome { path })
}

pub fn edit_content(root: &Path, request: &EditContentRequest) -> SiteResult<WriteOutcome> {
    let mut document = load_content_document(root, &request.target)?;

    set_required_string_if_some(&mut document.frontmatter, "title", request.title.clone());
    set_optional_string_if_some(
        &mut document.frontmatter,
        "description",
        request.description.clone(),
    );
    set_optional_string_if_some(&mut document.frontmatter, "slug", request.slug.clone());
    set_optional_string_if_some(&mut document.frontmatter, "url", request.url.clone());
    set_optional_string_if_some(&mut document.frontmatter, "date", request.date.clone());
    set_optional_string_if_some(&mut document.frontmatter, "author", request.author.clone());
    set_optional_string_if_some(
        &mut document.frontmatter,
        "repo_url",
        request.repo_url.clone(),
    );
    set_optional_string_if_some(
        &mut document.frontmatter,
        "live_url",
        request.live_url.clone(),
    );
    set_optional_string_if_some(&mut document.frontmatter, "status", request.status.clone());
    set_optional_string_if_some(&mut document.frontmatter, "icon", request.icon.clone());
    set_optional_string_if_some(
        &mut document.frontmatter,
        "target_page",
        request.target_page.clone(),
    );

    if let Some(order) = request.order {
        document
            .frontmatter
            .insert("order".into(), toml::Value::Integer(order as i64));
    }
    if let Some(weight) = request.weight {
        document
            .frontmatter
            .insert("weight".into(), toml::Value::Integer(weight as i64));
    }
    if let Some(tags) = &request.tags {
        set_string_array(&mut document.frontmatter, "tags", tags);
    }
    if let Some(categories) = &request.categories {
        set_string_array(&mut document.frontmatter, "categories", categories);
    }
    if let Some(tech_stack) = &request.tech_stack {
        set_string_array(&mut document.frontmatter, "tech_stack", tech_stack);
    }

    if let Some(slot) = &request.slot {
        let page_scope = request.page_scope.clone().unwrap_or_else(|| {
            string_value(&document.frontmatter, "page_scope").unwrap_or_else(|| "*".into())
        });
        validate_slot_scope(slot, &page_scope)?;
        document
            .frontmatter
            .insert("slot".into(), toml::Value::String(slot.clone()));
    }
    if let Some(page_scope) = &request.page_scope {
        let slot =
            string_value(&document.frontmatter, "slot").unwrap_or_else(|| "text-block".into());
        validate_slot_scope(&slot, page_scope)?;
        document
            .frontmatter
            .insert("page_scope".into(), toml::Value::String(page_scope.clone()));
    }

    if let Some(body) = &request.body {
        document.body = body.clone();
    }

    write_existing_document(&document)?;
    Ok(WriteOutcome {
        path: document.path,
    })
}

pub fn assign_slot(root: &Path, request: &AssignSlotRequest) -> SiteResult<WriteOutcome> {
    let mut document = load_content_document(root, &request.target)?;
    let page_scope = request.page_scope.clone().unwrap_or_else(|| {
        string_value(&document.frontmatter, "page_scope").unwrap_or_else(|| "*".into())
    });

    validate_slot_scope(&request.slot, &page_scope)?;

    document
        .frontmatter
        .insert("slot".into(), toml::Value::String(request.slot.clone()));
    document
        .frontmatter
        .insert("page_scope".into(), toml::Value::String(page_scope));

    if let Some(order) = request.order {
        document
            .frontmatter
            .insert("order".into(), toml::Value::Integer(order as i64));
    }
    if let Some(weight) = request.weight {
        document
            .frontmatter
            .insert("weight".into(), toml::Value::Integer(weight as i64));
    }

    write_existing_document(&document)?;
    Ok(WriteOutcome {
        path: document.path,
    })
}

pub fn load_content_document(root: &Path, selector: &str) -> SiteResult<ContentDocument> {
    let content_dir = content_dir(root)?;
    let resolved_path = resolve_content_target(root, &content_dir, selector)?;
    let raw = std::fs::read_to_string(&resolved_path)
        .map_err(io_with_path(&resolved_path, "reading content file"))?;
    let (yaml, body) = split_frontmatter(&raw).map_err(|err| match err {
        SiteError::Frontmatter { message, .. } => SiteError::Frontmatter {
            path: resolved_path.display().to_string(),
            message,
        },
        other => other,
    })?;
    let frontmatter =
        toml::from_str::<toml::Table>(yaml).map_err(|source| SiteError::Frontmatter {
            path: resolved_path.display().to_string(),
            message: source.to_string(),
        })?;
    let relative_path = resolved_path
        .strip_prefix(root)
        .unwrap_or(&resolved_path)
        .to_path_buf();

    Ok(ContentDocument {
        path: resolved_path,
        relative_path,
        frontmatter,
        body: body.to_string(),
    })
}

pub fn load_reorder_entries(
    root: &Path,
    slot_filter: &str,
    page_scope_filter: Option<&str>,
    query_filter: Option<&str>,
) -> SiteResult<Vec<ReorderEntry>> {
    let content_dir = content_dir(root)?;
    let page_scope_filter = page_scope_filter.map(|value| value.to_ascii_lowercase());
    let query_filter = query_filter
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());

    let mut entries = WalkDir::new(&content_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "md"))
        .filter_map(|entry| {
            let path = entry.path().to_path_buf();
            let relative_path = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            let raw = std::fs::read_to_string(&path).ok()?;
            let (yaml, _) = split_frontmatter(&raw).ok()?;
            let frontmatter = toml::from_str::<toml::Table>(yaml).ok()?;
            let slot = string_value(&frontmatter, "slot")?;
            if slot != slot_filter {
                return None;
            }

            let title = string_value(&frontmatter, "title").unwrap_or_else(|| {
                path.file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("untitled")
                    .to_string()
            });
            let page_scope = string_value(&frontmatter, "page_scope").unwrap_or_else(|| "*".into());
            if let Some(page_scope_filter) = &page_scope_filter {
                if page_scope.to_ascii_lowercase() != *page_scope_filter {
                    return None;
                }
            }

            if let Some(query_filter) = &query_filter {
                let slug = string_value(&frontmatter, "slug").unwrap_or_default();
                let rel = relative_path.to_string_lossy().to_ascii_lowercase();
                let title_lower = title.to_ascii_lowercase();
                let slug_lower = slug.to_ascii_lowercase();
                if !rel.contains(query_filter)
                    && !title_lower.contains(query_filter)
                    && !slug_lower.contains(query_filter)
                {
                    return None;
                }
            }

            Some(ReorderEntry {
                path,
                relative_path,
                title,
                slot,
                page_scope,
                order: integer_value(&frontmatter, "order").unwrap_or(0),
                weight: integer_value(&frontmatter, "weight").unwrap_or(50),
            })
        })
        .collect::<Vec<_>>();

    entries.sort_by(|a, b| {
        a.order
            .cmp(&b.order)
            .then(b.weight.cmp(&a.weight))
            .then(a.title.cmp(&b.title))
    });

    Ok(entries)
}

pub fn move_reorder_entry(
    entries: &mut Vec<ReorderEntry>,
    from_index: usize,
    to_index: usize,
) -> SiteResult<()> {
    if entries.is_empty() {
        return Err(SiteError::Build("There are no entries to reorder.".into()));
    }
    if from_index >= entries.len() || to_index >= entries.len() {
        return Err(SiteError::Build(format!(
            "Move indices must be between 1 and {}.",
            entries.len()
        )));
    }
    if from_index == to_index {
        return Ok(());
    }

    let entry = entries.remove(from_index);
    entries.insert(to_index, entry);
    Ok(())
}

pub fn persist_reordered_entries(
    root: &Path,
    entries: &[ReorderEntry],
    start_at: i32,
    step: i32,
) -> SiteResult<Vec<WriteOutcome>> {
    if step <= 0 {
        return Err(SiteError::Build(
            "Reorder step must be greater than zero.".into(),
        ));
    }

    let mut outcomes = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let mut document = load_content_document(root, &entry.relative_path.to_string_lossy())?;
        let order = start_at + (index as i32 * step);
        document
            .frontmatter
            .insert("order".into(), toml::Value::Integer(order as i64));
        write_existing_document(&document)?;
        outcomes.push(WriteOutcome {
            path: document.path.clone(),
        });
    }

    Ok(outcomes)
}

fn resolve_content_target(root: &Path, content_dir: &Path, selector: &str) -> SiteResult<PathBuf> {
    let direct_candidates = [
        root.join(selector),
        content_dir.join(selector),
        root.join(format!("{}.md", selector)),
        content_dir.join(format!("{}.md", selector)),
    ];

    for candidate in direct_candidates {
        if candidate.exists() && candidate.extension().is_some_and(|ext| ext == "md") {
            return Ok(candidate);
        }
    }

    let selector_lower = selector.to_ascii_lowercase();
    let matches = WalkDir::new(content_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "md"))
        .filter_map(|entry| {
            let path = entry.path().to_path_buf();
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let stem = path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let raw = std::fs::read_to_string(&path).ok()?;
            let slug = split_frontmatter(&raw)
                .ok()
                .and_then(|(yaml, _)| toml::from_str::<toml::Table>(yaml).ok())
                .and_then(|table| string_value(&table, "slug"));
            let title = split_frontmatter(&raw)
                .ok()
                .and_then(|(yaml, _)| toml::from_str::<toml::Table>(yaml).ok())
                .and_then(|table| string_value(&table, "title"))
                .map(|value| value.to_ascii_lowercase());

            let selector_matches = rel_str.eq_ignore_ascii_case(selector)
                || rel_str.eq_ignore_ascii_case(&format!("content/{}", selector))
                || rel_str.eq_ignore_ascii_case(&format!("{}.md", selector))
                || rel_str.eq_ignore_ascii_case(&format!("content/{}.md", selector))
                || stem == selector_lower
                || slug
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(selector))
                || title
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(&selector_lower));

            selector_matches.then_some(path)
        })
        .collect::<Vec<_>>();

    match matches.len() {
        0 => Err(SiteError::Build(format!(
            "Could not find a content file matching '{}'. Pass a path like 'content/about.md' or a unique slug/title.",
            selector
        ))),
        1 => Ok(matches.into_iter().next().expect("single match")),
        _ => Err(SiteError::Build(format!(
            "Selector '{}' matched multiple content files: {}",
            selector,
            matches
                .into_iter()
                .map(|path| path.strip_prefix(root).unwrap_or(&path).display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

fn content_dir(root: &Path) -> SiteResult<PathBuf> {
    let config = load_site_config_for_root(root)?;
    Ok(root.join(config.build.content_dir))
}

fn defaulted_output_path(
    root: &Path,
    provided_path: Option<&PathBuf>,
    fallback_relative_path: &str,
) -> SiteResult<PathBuf> {
    let content_dir = content_dir(root)?;
    let path = provided_path
        .cloned()
        .map(|path| {
            if path.is_absolute() {
                path
            } else if path.starts_with("content") {
                root.join(path)
            } else {
                content_dir.join(path)
            }
        })
        .unwrap_or_else(|| content_dir.join(fallback_relative_path));

    if path.exists() {
        return Err(SiteError::Build(format!(
            "Refusing to overwrite existing file '{}'. Use 'ferrosite edit' to modify it or choose a different path.",
            path.display()
        )));
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(io_with_path(parent, "creating content directory"))?;
    }

    Ok(path)
}

fn write_new_document(
    path: &Path,
    frontmatter: toml::map::Map<String, toml::Value>,
    body: &str,
) -> SiteResult<()> {
    let rendered = render_document(&frontmatter, body)?;
    std::fs::write(path, rendered).map_err(io_with_path(path, "writing content file"))
}

fn write_existing_document(document: &ContentDocument) -> SiteResult<()> {
    let rendered = render_document(&document.frontmatter, &document.body)?;
    std::fs::write(&document.path, rendered)
        .map_err(io_with_path(&document.path, "writing content file"))
}

fn render_document(
    frontmatter: &toml::map::Map<String, toml::Value>,
    body: &str,
) -> SiteResult<String> {
    let frontmatter_str = toml::to_string_pretty(frontmatter)?;
    let trimmed_body = body.trim_end();
    let mut rendered = format!("---\n{}---\n", frontmatter_str);
    if !trimmed_body.is_empty() {
        rendered.push('\n');
        rendered.push_str(trimmed_body);
        rendered.push('\n');
    }
    Ok(rendered)
}

fn validate_slot_scope(slot: &str, page_scope: &str) -> SiteResult<SlotType> {
    let slot_type = slot
        .parse::<SlotType>()
        .ok()
        .ok_or_else(|| SiteError::UnknownSlot {
            slot: slot.to_string(),
            path: "<interactive>".into(),
        })?;

    if page_scope == "*" {
        return Ok(slot_type);
    }

    let allowed = slot_type.allowed_page_types();
    if allowed.contains(&"*") || allowed.contains(&page_scope) {
        Ok(slot_type)
    } else {
        Err(SiteError::Build(format!(
            "Slot '{}' does not normally render on page_scope '{}'. Allowed scopes: {}",
            slot,
            page_scope,
            allowed.join(", ")
        )))
    }
}

fn should_write_slug(slot: &str) -> bool {
    matches!(slot, "article-body" | "project-body")
}

fn default_page_path(slug: &str, slot: &str, page_scope: &str) -> String {
    match (slot, page_scope) {
        ("about-body", "about") => "about.md".into(),
        ("hero", "home") => "home.md".into(),
        ("contact-form", _) => "contact-form.md".into(),
        _ => format!("{}.md", slug),
    }
}

fn slug_or_default(provided_slug: Option<&str>, title: &str) -> String {
    let slug = provided_slug
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| slugify(title));

    if slug.is_empty() {
        "untitled".into()
    } else {
        slug
    }
}

fn string_value(table: &toml::map::Map<String, toml::Value>, key: &str) -> Option<String> {
    table
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn integer_value(table: &toml::map::Map<String, toml::Value>, key: &str) -> Option<i32> {
    table
        .get(key)
        .and_then(|value| value.as_integer())
        .map(|value| value as i32)
}

fn set_optional_string(
    table: &mut toml::map::Map<String, toml::Value>,
    key: &str,
    value: Option<String>,
) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        table.insert(key.into(), toml::Value::String(value));
    }
}

fn set_required_string_if_some(
    table: &mut toml::map::Map<String, toml::Value>,
    key: &str,
    value: Option<String>,
) {
    if let Some(value) = value {
        table.insert(key.into(), toml::Value::String(value));
    }
}

fn set_optional_string_if_some(
    table: &mut toml::map::Map<String, toml::Value>,
    key: &str,
    value: Option<String>,
) {
    if let Some(value) = value {
        if value.trim().is_empty() {
            table.remove(key);
        } else {
            table.insert(key.into(), toml::Value::String(value));
        }
    }
}

fn set_string_array(table: &mut toml::map::Map<String, toml::Value>, key: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }

    table.insert(
        key.into(),
        toml::Value::Array(
            values
                .iter()
                .filter(|value| !value.trim().is_empty())
                .cloned()
                .map(toml::Value::String)
                .collect(),
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scaffold_site(root: &Path) {
        std::fs::create_dir_all(root.join("content")).expect("content dir");
        std::fs::write(
            root.join("ferrosite.toml"),
            r#"[site]
title = "Example"
description = "Example site"
base_url = "https://example.com"

[site.author]
name = "Jane Doe"

[build]
template = "developer"
content_dir = "content"
output_dir = "dist"
assets_dir = "assets"

[deploy]
provider = "cloudflare"

[deploy.cloudflare]
project_name = "example"
account_id = "abc123"
"#,
        )
        .expect("config");
    }

    #[test]
    fn create_article_writes_blog_post_frontmatter() {
        let temp = tempfile::tempdir().expect("tempdir");
        scaffold_site(temp.path());

        let outcome = create_article(
            temp.path(),
            &NewArticleRequest {
                title: "Hello Rust".into(),
                slug: None,
                description: Some("A short post".into()),
                author: Some("Jane Doe".into()),
                date: "2026-04-05".into(),
                tags: vec!["rust".into(), "cli".into()],
                categories: vec![],
                featured: true,
                draft: false,
                path: None,
                body: "Hello world".into(),
            },
        )
        .expect("article");

        assert!(outcome.path.ends_with("content/blog/hello-rust.md"));
        let raw = std::fs::read_to_string(&outcome.path).expect("file");
        assert!(raw.contains("slot = \"article-body\""));
        assert!(raw.contains("page_scope = \"blog\""));
        assert!(raw.contains("featured = true"));
        assert!(raw.contains("Hello world"));
    }

    #[test]
    fn create_page_can_generate_companion_nav_entry() {
        let temp = tempfile::tempdir().expect("tempdir");
        scaffold_site(temp.path());

        let outcome = create_page(
            temp.path(),
            &NewPageRequest {
                title: "About".into(),
                slug: None,
                description: Some("About page body".into()),
                slot: "about-body".into(),
                page_scope: "about".into(),
                url: Some("/about/".into()),
                order: 0,
                weight: 50,
                path: None,
                body: "About body".into(),
                nav: Some(NewNavRequest {
                    title: "About".into(),
                    url: "/about/".into(),
                    order: 2,
                    weight: 50,
                    icon: Some("02".into()),
                    external: false,
                    target_page: Some("about".into()),
                    path: None,
                }),
            },
        )
        .expect("page");

        assert!(outcome.page_path.ends_with("content/about.md"));
        let nav_path = outcome.nav_path.expect("nav path");
        assert!(nav_path.ends_with("content/nav-about.md"));

        let nav_raw = std::fs::read_to_string(nav_path).expect("nav file");
        assert!(nav_raw.contains("slot = \"nav-item\""));
        assert!(nav_raw.contains("target_page = \"about\""));
    }

    #[test]
    fn edit_content_resolves_by_slug_and_updates_frontmatter() {
        let temp = tempfile::tempdir().expect("tempdir");
        scaffold_site(temp.path());
        std::fs::create_dir_all(temp.path().join("content/blog")).expect("blog dir");
        std::fs::write(
            temp.path().join("content/blog/hello-rust.md"),
            r#"---
title = "Hello Rust"
slug = "hello-rust"
slot = "article-body"
page_scope = "blog"
date = "2026-04-01"
---

Body
"#,
        )
        .expect("content");

        let outcome = edit_content(
            temp.path(),
            &EditContentRequest {
                target: "hello-rust".into(),
                title: Some("Hello Ferrosite".into()),
                description: Some("Updated description".into()),
                ..EditContentRequest::default()
            },
        )
        .expect("edit");

        let updated = std::fs::read_to_string(outcome.path).expect("updated file");
        assert!(updated.contains("title = \"Hello Ferrosite\""));
        assert!(updated.contains("description = \"Updated description\""));
        assert!(updated.contains("Body"));
    }

    #[test]
    fn assign_slot_updates_slot_and_scope() {
        let temp = tempfile::tempdir().expect("tempdir");
        scaffold_site(temp.path());
        std::fs::write(
            temp.path().join("content/home.md"),
            r#"---
title = "Home"
slot = "text-block"
page_scope = "home"
---

Intro
"#,
        )
        .expect("content");

        assign_slot(
            temp.path(),
            &AssignSlotRequest {
                target: "content/home.md".into(),
                slot: "hero".into(),
                page_scope: Some("home".into()),
                order: Some(1),
                weight: Some(90),
            },
        )
        .expect("assign slot");

        let updated = std::fs::read_to_string(temp.path().join("content/home.md")).expect("file");
        assert!(updated.contains("slot = \"hero\""));
        assert!(updated.contains("page_scope = \"home\""));
        assert!(updated.contains("order = 1"));
        assert!(updated.contains("weight = 90"));
    }

    #[test]
    fn load_reorder_entries_filters_by_slot_and_scope() {
        let temp = tempfile::tempdir().expect("tempdir");
        scaffold_site(temp.path());
        std::fs::write(
            temp.path().join("content/nav-home.md"),
            r#"---
title = "Home"
slot = "nav-item"
page_scope = "*"
order = 10
---
"#,
        )
        .expect("nav home");
        std::fs::write(
            temp.path().join("content/nav-about.md"),
            r#"---
title = "About"
slot = "nav-item"
page_scope = "*"
order = 20
---
"#,
        )
        .expect("nav about");
        std::fs::write(
            temp.path().join("content/home.md"),
            r#"---
title = "Hero"
slot = "hero"
page_scope = "home"
order = 0
---
"#,
        )
        .expect("hero");

        let entries = load_reorder_entries(temp.path(), "nav-item", Some("*"), None)
            .expect("reorder entries");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Home");
        assert_eq!(entries[1].title, "About");
    }

    #[test]
    fn move_reorder_entry_repositions_items() {
        let mut entries = vec![
            ReorderEntry {
                path: PathBuf::from("content/nav-home.md"),
                relative_path: PathBuf::from("content/nav-home.md"),
                title: "Home".into(),
                slot: "nav-item".into(),
                page_scope: "*".into(),
                order: 10,
                weight: 50,
            },
            ReorderEntry {
                path: PathBuf::from("content/nav-about.md"),
                relative_path: PathBuf::from("content/nav-about.md"),
                title: "About".into(),
                slot: "nav-item".into(),
                page_scope: "*".into(),
                order: 20,
                weight: 50,
            },
            ReorderEntry {
                path: PathBuf::from("content/nav-blog.md"),
                relative_path: PathBuf::from("content/nav-blog.md"),
                title: "Blog".into(),
                slot: "nav-item".into(),
                page_scope: "*".into(),
                order: 30,
                weight: 50,
            },
        ];

        move_reorder_entry(&mut entries, 2, 0).expect("move entry");

        assert_eq!(entries[0].title, "Blog");
        assert_eq!(entries[1].title, "Home");
        assert_eq!(entries[2].title, "About");
    }

    #[test]
    fn persist_reordered_entries_rewrites_order_fields() {
        let temp = tempfile::tempdir().expect("tempdir");
        scaffold_site(temp.path());
        std::fs::write(
            temp.path().join("content/nav-home.md"),
            r#"---
title = "Home"
slot = "nav-item"
page_scope = "*"
order = 30
---
"#,
        )
        .expect("nav home");
        std::fs::write(
            temp.path().join("content/nav-about.md"),
            r#"---
title = "About"
slot = "nav-item"
page_scope = "*"
order = 10
---
"#,
        )
        .expect("nav about");

        let entries = vec![
            ReorderEntry {
                path: temp.path().join("content/nav-home.md"),
                relative_path: PathBuf::from("content/nav-home.md"),
                title: "Home".into(),
                slot: "nav-item".into(),
                page_scope: "*".into(),
                order: 30,
                weight: 50,
            },
            ReorderEntry {
                path: temp.path().join("content/nav-about.md"),
                relative_path: PathBuf::from("content/nav-about.md"),
                title: "About".into(),
                slot: "nav-item".into(),
                page_scope: "*".into(),
                order: 10,
                weight: 50,
            },
        ];

        persist_reordered_entries(temp.path(), &entries, 10, 10).expect("persist reorder");

        let home = std::fs::read_to_string(temp.path().join("content/nav-home.md")).expect("home");
        let about =
            std::fs::read_to_string(temp.path().join("content/nav-about.md")).expect("about");
        assert!(home.contains("order = 10"));
        assert!(about.contains("order = 20"));
    }
}
