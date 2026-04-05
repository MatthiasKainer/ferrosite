use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{io_with_path, SiteError, SiteResult};

// ── Site Config ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfig {
    pub site: SiteMetadata,
    pub build: BuildConfig,
    pub deploy: DeployConfig,
    #[serde(default)]
    pub layout: LayoutConfig,
    #[serde(default)]
    pub routes: RouteConfig,
    #[serde(default)]
    pub plugins: PluginConfig,
    #[serde(default)]
    pub extra: HashMap<String, toml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteMetadata {
    pub title: String,
    pub description: String,
    pub base_url: String,
    pub author: AuthorConfig,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub favicon: Option<String>,
    #[serde(default)]
    pub social: SocialConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthorConfig {
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub bio: Option<String>,
    #[serde(default)]
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SocialConfig {
    #[serde(default)]
    pub github: Option<String>,
    #[serde(default)]
    pub linkedin: Option<String>,
    #[serde(default)]
    pub twitter: Option<String>,
    #[serde(default)]
    pub mastodon: Option<String>,
    #[serde(default)]
    pub website: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    pub template: String,
    #[serde(default = "default_content_dir")]
    pub content_dir: String,
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
    #[serde(default = "default_assets_dir")]
    pub assets_dir: String,
    #[serde(default)]
    pub ssr: SsrConfig,
    #[serde(default = "default_true")]
    pub minify: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsrConfig {
    /// Run Puppeteer SSR pass after HTML generation
    #[serde(default)]
    pub enabled: bool,
    /// Path to node executable (default: "node")
    #[serde(default = "default_node")]
    pub node_bin: String,
    /// Package manager binary used by `ferrosite ssr-setup` (default: "npm")
    #[serde(default = "default_package_manager")]
    pub package_manager_bin: String,
    /// Timeout per page in ms
    #[serde(default = "default_ssr_timeout")]
    pub timeout_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    /// Whether to include a top navigation menu
    #[serde(default = "default_true")]
    pub menu: bool,
    /// Whether to include a macOS-style dock
    #[serde(default)]
    pub dock: bool,
    /// Whether to include a sidebar on content pages
    #[serde(default)]
    pub sidebar: bool,
    /// Sidebar position: "left" | "right"
    #[serde(default = "default_sidebar_position")]
    pub sidebar_position: String,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            menu: true,
            dock: false,
            sidebar: false,
            sidebar_position: "right".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteConfig {
    #[serde(default = "default_blog_post_path")]
    pub blog_post_path: String,
    #[serde(default = "default_projects_path")]
    pub projects_path: String,
}

impl Default for RouteConfig {
    fn default() -> Self {
        Self {
            blog_post_path: default_blog_post_path(),
            projects_path: default_projects_path(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginConfig {
    #[serde(default)]
    pub plugins_dir: Option<String>,
    #[serde(default)]
    pub enabled: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployConfig {
    pub provider: DeployProvider,
    #[serde(default)]
    pub cloudflare: Option<CloudflareDeployConfig>,
    #[serde(default)]
    pub aws: Option<AwsDeployConfig>,
    #[serde(default)]
    pub azure: Option<AzureDeployConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeployProvider {
    Cloudflare,
    Aws,
    Azure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflareDeployConfig {
    /// Cloudflare Pages project name
    pub project_name: String,
    /// Cloudflare account ID
    pub account_id: String,
    /// Workers subdomain prefix for plugin lambdas
    #[serde(default)]
    pub workers_subdomain: Option<String>,
    /// Custom domain (optional)
    #[serde(default)]
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsDeployConfig {
    pub bucket_name: String,
    pub region: String,
    #[serde(default)]
    pub cloudfront_distribution_id: Option<String>,
    #[serde(default)]
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureDeployConfig {
    pub resource_group: String,
    pub app_name: String,
    #[serde(default)]
    pub subscription_id: Option<String>,
}

// ── Theme Config ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThemeConfig {
    #[serde(default)]
    pub colors: ColorTokens,
    #[serde(default)]
    pub typography: TypographyTokens,
    #[serde(default)]
    pub spacing: SpacingTokens,
    #[serde(default)]
    pub extra: HashMap<String, toml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorTokens {
    pub primary: String,
    pub primary_dark: String,
    pub accent: String,
    pub background: String,
    pub surface: String,
    pub text: String,
    pub text_muted: String,
    pub border: String,
    pub code_bg: String,
    pub success: String,
    pub warning: String,
    pub error: String,
}

impl Default for ColorTokens {
    fn default() -> Self {
        Self {
            primary: "#0ea5e9".into(),
            primary_dark: "#0284c7".into(),
            accent: "#8b5cf6".into(),
            background: "#0f172a".into(),
            surface: "#1e293b".into(),
            text: "#f1f5f9".into(),
            text_muted: "#94a3b8".into(),
            border: "#334155".into(),
            code_bg: "#0f172a".into(),
            success: "#22c55e".into(),
            warning: "#f59e0b".into(),
            error: "#ef4444".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypographyTokens {
    pub font_sans: String,
    pub font_mono: String,
    pub font_heading: String,
    pub size_base: String,
    pub size_lg: String,
    pub size_xl: String,
    pub size_2xl: String,
    pub size_3xl: String,
    pub line_height: String,
}

impl Default for TypographyTokens {
    fn default() -> Self {
        Self {
            font_sans: "'Inter', system-ui, -apple-system, sans-serif".into(),
            font_mono: "'JetBrains Mono', 'Fira Code', monospace".into(),
            font_heading: "'Inter', system-ui, sans-serif".into(),
            size_base: "1rem".into(),
            size_lg: "1.125rem".into(),
            size_xl: "1.25rem".into(),
            size_2xl: "1.5rem".into(),
            size_3xl: "1.875rem".into(),
            line_height: "1.6".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpacingTokens {
    pub unit: String,
    pub container_max: String,
    pub sidebar_width: String,
    pub header_height: String,
    pub dock_height: String,
}

impl Default for SpacingTokens {
    fn default() -> Self {
        Self {
            unit: "0.25rem".into(),
            container_max: "1200px".into(),
            sidebar_width: "280px".into(),
            header_height: "64px".into(),
            dock_height: "72px".into(),
        }
    }
}

// ── Loaders ────────────────────────────────────────────────────────────────────

/// Load and validate site config from a TOML file — pure after IO.
pub fn load_site_config(path: &Path) -> SiteResult<SiteConfig> {
    std::fs::read_to_string(path)
        .map_err(io_with_path(path, "reading site config"))
        .and_then(|raw| toml::from_str::<SiteConfig>(&raw).map_err(SiteError::from))
        .and_then(validate_site_config)
}

/// Resolve the site config for a root directory with an actionable error when
/// no `ferrosite.toml` exists there.
pub fn load_site_config_for_root(site_root: &Path) -> SiteResult<SiteConfig> {
    let config_path = site_root.join("ferrosite.toml");
    if config_path.exists() {
        return load_site_config(&config_path);
    }

    Err(SiteError::Config(format!(
        "No ferrosite.toml found in '{}'. Run 'ferrosite new <name>' to scaffold a site or pass '--root <site-dir>' to point at an existing site.",
        site_root.display()
    )))
}

/// Pure validation of SiteConfig fields.
fn validate_site_config(config: SiteConfig) -> SiteResult<SiteConfig> {
    if config.site.title.is_empty() {
        return Err(SiteError::MissingConfig {
            field: "site.title".into(),
            file: "ferrosite.toml".into(),
        });
    }
    if config.site.base_url.is_empty() {
        return Err(SiteError::MissingConfig {
            field: "site.base_url".into(),
            file: "ferrosite.toml".into(),
        });
    }
    Ok(config)
}

/// Load theme config from a TOML file.
pub fn load_theme_config(path: &Path) -> SiteResult<ThemeConfig> {
    if !path.exists() {
        return Ok(ThemeConfig::default());
    }
    std::fs::read_to_string(path)
        .map_err(io_with_path(path, "reading theme config"))
        .and_then(|raw| toml::from_str::<ThemeConfig>(&raw).map_err(SiteError::from))
}

/// Convert ThemeConfig to CSS custom properties string — pure function.
pub fn theme_to_css_vars(theme: &ThemeConfig) -> String {
    format!(
        r#":root {{
  --color-primary: {primary};
  --color-primary-dark: {primary_dark};
  --color-accent: {accent};
  --color-bg: {background};
  --color-surface: {surface};
  --color-text: {text};
  --color-text-muted: {text_muted};
  --color-border: {border};
  --color-code-bg: {code_bg};
  --color-success: {success};
  --color-warning: {warning};
  --color-error: {error};
  --font-sans: {font_sans};
  --font-mono: {font_mono};
  --font-heading: {font_heading};
  --font-size-base: {size_base};
  --font-size-lg: {size_lg};
  --font-size-xl: {size_xl};
  --font-size-2xl: {size_2xl};
  --font-size-3xl: {size_3xl};
  --line-height: {line_height};
  --spacing-unit: {unit};
  --container-max: {container_max};
  --sidebar-width: {sidebar_width};
  --header-height: {header_height};
  --dock-height: {dock_height};
}}"#,
        primary = theme.colors.primary,
        primary_dark = theme.colors.primary_dark,
        accent = theme.colors.accent,
        background = theme.colors.background,
        surface = theme.colors.surface,
        text = theme.colors.text,
        text_muted = theme.colors.text_muted,
        border = theme.colors.border,
        code_bg = theme.colors.code_bg,
        success = theme.colors.success,
        warning = theme.colors.warning,
        error = theme.colors.error,
        font_sans = theme.typography.font_sans,
        font_mono = theme.typography.font_mono,
        font_heading = theme.typography.font_heading,
        size_base = theme.typography.size_base,
        size_lg = theme.typography.size_lg,
        size_xl = theme.typography.size_xl,
        size_2xl = theme.typography.size_2xl,
        size_3xl = theme.typography.size_3xl,
        line_height = theme.typography.line_height,
        unit = theme.spacing.unit,
        container_max = theme.spacing.container_max,
        sidebar_width = theme.spacing.sidebar_width,
        header_height = theme.spacing.header_height,
        dock_height = theme.spacing.dock_height,
    )
}

// ── Defaults ───────────────────────────────────────────────────────────────────

fn default_content_dir() -> String {
    "content".into()
}
fn default_output_dir() -> String {
    "dist".into()
}
fn default_assets_dir() -> String {
    "assets".into()
}
fn default_true() -> bool {
    true
}

fn default_blog_post_path() -> String {
    "blog".into()
}

fn default_projects_path() -> String {
    "projects".into()
}
fn default_node() -> String {
    "node".into()
}
fn default_package_manager() -> String {
    "npm".into()
}
fn default_ssr_timeout() -> u32 {
    30_000
}
fn default_sidebar_position() -> String {
    "right".into()
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            template: "developer".into(),
            content_dir: default_content_dir(),
            output_dir: default_output_dir(),
            assets_dir: default_assets_dir(),
            ssr: SsrConfig::default(),
            minify: true,
        }
    }
}

impl Default for SsrConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            node_bin: default_node(),
            package_manager_bin: default_package_manager(),
            timeout_ms: default_ssr_timeout(),
        }
    }
}

/// Determine the template directory from site root + template name.
pub fn template_dir(site_root: &Path, template_name: &str) -> PathBuf {
    // First check site-local templates/, then fall-back to crate-bundled templates/
    let local = site_root.join("templates").join(template_name);
    if local.exists() {
        return local;
    }
    // Bundled (relative to cargo workspace for dev / installed binary for release)
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("templates")
        .join(template_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn ssr_package_manager_bin_defaults_to_npm() {
        let raw = r#"
[site]
title = "Example"
description = "Example site"
base_url = "https://example.com"

[site.author]
name = "Example Author"

[build]
template = "developer"

[deploy]
provider = "cloudflare"

[deploy.cloudflare]
project_name = "example"
account_id = "abc123"
"#;

        let config: SiteConfig = toml::from_str(raw).expect("config should deserialize");
        assert_eq!(config.build.ssr.package_manager_bin, "npm");
    }

    #[test]
    fn ssr_package_manager_bin_can_be_overridden() {
        let raw = r#"
[site]
title = "Example"
description = "Example site"
base_url = "https://example.com"

[site.author]
name = "Example Author"

[build]
template = "developer"

[build.ssr]
package_manager_bin = "pnpm"

[deploy]
provider = "cloudflare"

[deploy.cloudflare]
project_name = "example"
account_id = "abc123"
"#;

        let config: SiteConfig = toml::from_str(raw).expect("config should deserialize");
        assert_eq!(config.build.ssr.package_manager_bin, "pnpm");
    }

    #[test]
    fn missing_site_config_in_root_returns_actionable_error() {
        let temp = tempdir().expect("tempdir should be created");
        let err = load_site_config_for_root(temp.path()).expect_err("missing config should error");

        match err {
            SiteError::Config(message) => {
                assert!(message.contains("No ferrosite.toml found"));
                assert!(message.contains("ferrosite new <name>"));
                assert!(message.contains("--root <site-dir>"));
            }
            other => panic!("expected config error, got {other:?}"),
        }
    }

    #[test]
    fn theme_to_css_vars_uses_theme_tokens() {
        let mut theme = ThemeConfig::default();
        theme.colors.primary = "#123456".into();
        theme.typography.font_heading = "'Fraunces', serif".into();
        theme.spacing.sidebar_width = "320px".into();

        let css = theme_to_css_vars(&theme);

        assert!(css.contains("--color-primary: #123456;"));
        assert!(css.contains("--font-heading: 'Fraunces', serif;"));
        assert!(css.contains("--sidebar-width: 320px;"));
    }

    #[test]
    fn template_dir_prefers_site_local_template_when_present() {
        let temp = tempdir().expect("tempdir should be created");
        let local = temp.path().join("templates").join("custom");
        std::fs::create_dir_all(&local).expect("local template dir should be created");

        let resolved = template_dir(temp.path(), "custom");

        assert_eq!(resolved, local);
    }
}
