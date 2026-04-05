use thiserror::Error;

#[derive(Debug, Error)]
pub enum SiteError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Missing required config field '{field}' in '{file}'")]
    MissingConfig { field: String, file: String },
    #[error("Content error in '{path}': {message}")]
    Content { path: String, message: String },
    #[error("Frontmatter parse error in '{path}': {message}")]
    Frontmatter { path: String, message: String },
    #[error("Unknown slot type '{slot}' in '{path}'")]
    UnknownSlot { slot: String, path: String },
    #[error("Article '{path}' missing required field '{field}' for slot '{slot}'")]
    MissingArticleField {
        path: String,
        field: String,
        slot: String,
    },
    #[error("Template '{template}' not found")]
    TemplateNotFound { template: String },
    #[error("Template render error in '{template}': {message}")]
    TemplateRenderMsg { template: String, message: String },
    #[error("Layout '{layout}' not found for page type '{page_type}'")]
    LayoutNotFound { layout: String, page_type: String },
    #[error("SSR error: {0}")]
    Ssr(String),
    #[error("Component error in '{component}': {message}")]
    Component { component: String, message: String },
    #[error("Plugin manifest error in '{plugin}': {message}")]
    Plugin { plugin: String, message: String },
    #[error("Plugin slot conflict: plugin '{plugin}' claims slot '{slot}'")]
    PluginSlotConflict { plugin: String, slot: String },
    #[error("Build error: {0}")]
    Build(String),
    #[error("Asset error for '{asset}': {message}")]
    Asset { asset: String, message: String },
    #[error("Deploy error for provider '{provider}': {message}")]
    Deploy { provider: String, message: String },
    #[error("Missing deploy credential: {0}")]
    MissingCredential(String),
    #[error("Missing path while {action}: '{path}'")]
    MissingPath { action: String, path: String },
    #[error("IO error while {action} '{path}': {source}")]
    IoPath {
        action: String,
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("IO error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },
    #[error("TOML deserialize error: {source}")]
    TomlDeserialize {
        #[from]
        source: toml::de::Error,
    },
    #[error("TOML serialize error: {source}")]
    TomlSerialize {
        #[from]
        source: toml::ser::Error,
    },
    #[error("JSON error: {source}")]
    Json {
        #[from]
        source: serde_json::Error,
    },
    #[error("Template engine error: {source}")]
    TemplateEngine {
        #[from]
        source: minijinja::Error,
    },
    #[error("Directory walk error: {source}")]
    WalkDir {
        #[from]
        source: walkdir::Error,
    },
    #[error("Path strip prefix error: {source}")]
    StripPrefix {
        #[from]
        source: std::path::StripPrefixError,
    },
    #[error("Multiple errors:\n{}", .0.iter().enumerate().map(|(i,e)| format!("  {}: {}", i+1, e)).collect::<Vec<_>>().join("\n"))]
    Multiple(Vec<SiteError>),
}

pub type SiteResult<T> = Result<T, SiteError>;

pub fn io_with_path(
    path: impl AsRef<std::path::Path>,
    action: impl Into<String>,
) -> impl FnOnce(std::io::Error) -> SiteError {
    let path = path.as_ref().display().to_string();
    let action = action.into();

    move |source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            SiteError::MissingPath {
                action,
                path: path.clone(),
            }
        } else {
            SiteError::IoPath {
                action,
                path,
                source,
            }
        }
    }
}

pub fn collect_results<T>(results: Vec<SiteResult<T>>) -> SiteResult<Vec<T>> {
    let (oks, errs): (Vec<_>, Vec<_>) = results.into_iter().partition(Result::is_ok);
    if errs.is_empty() {
        Ok(oks.into_iter().map(|r| r.unwrap()).collect())
    } else {
        let errors: Vec<SiteError> = errs
            .into_iter()
            .map(|r| match r {
                Ok(_) => unreachable!("partition(Result::is_ok) guarantees only errors here"),
                Err(err) => err,
            })
            .collect();
        if errors.len() == 1 {
            Err(errors.into_iter().next().unwrap())
        } else {
            Err(SiteError::Multiple(errors))
        }
    }
}

pub fn with_path<T>(path: &str, result: SiteResult<T>) -> SiteResult<T> {
    result.map_err(|e| SiteError::Content {
        path: path.to_string(),
        message: e.to_string(),
    })
}
