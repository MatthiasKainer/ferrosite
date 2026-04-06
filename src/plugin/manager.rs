use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;
use walkdir::WalkDir;

use crate::config::load_site_config_for_root;
use crate::error::{SiteError, SiteResult};

use super::{bundled_plugins_dir, site_plugins_dir, Plugin};

#[derive(Debug, Clone)]
pub struct PluginInstallOutcome {
    pub plugin_name: String,
    pub install_dir: PathBuf,
    pub already_installed: bool,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct PluginUninstallOutcome {
    pub plugin_name: String,
    pub removed_dir: Option<PathBuf>,
    pub disabled_only: bool,
    pub usage_files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct InstalledPlugin {
    plugin: Plugin,
    dir_name: String,
}

#[derive(Debug, Clone)]
struct PluginDirConfig {
    dir: PathBuf,
    configured_value: Option<String>,
}

pub fn install_plugin(site_root: &Path, source: &str) -> SiteResult<PluginInstallOutcome> {
    let plugin_dir_config = resolve_plugin_dir(site_root)?;

    let staged = if is_git_source(source) {
        std::fs::create_dir_all(&plugin_dir_config.dir)?;
        let temp = TempDir::new()?;
        let checkout_dir = temp.path().join("plugin");
        clone_plugin_repo(source, &checkout_dir)?;
        StagedPlugin {
            plugin: Plugin::from_dir(&checkout_dir)?,
            source: format!("git clone {}", source),
            _temp_dir: Some(temp),
            source_dir: checkout_dir,
        }
    } else {
        let source_dir = find_bundled_plugin_dir(source)?;
        StagedPlugin {
            plugin: Plugin::from_dir(&source_dir)?,
            source: format!("bundled plugin {}", source),
            _temp_dir: None,
            source_dir,
        }
    };

    if let Some(existing) =
        find_installed_plugin(&plugin_dir_config.dir, &staged.plugin.manifest.name)?
    {
        persist_plugin_enabled(
            site_root,
            &existing.plugin.manifest.name,
            plugin_dir_config.configured_value.as_deref(),
        )?;

        return Ok(PluginInstallOutcome {
            plugin_name: existing.plugin.manifest.name,
            install_dir: existing.plugin.dir,
            already_installed: true,
            source: staged.source,
        });
    }

    if !is_git_source(source) {
        persist_plugin_enabled(
            site_root,
            &staged.plugin.manifest.name,
            plugin_dir_config.configured_value.as_deref(),
        )?;

        return Ok(PluginInstallOutcome {
            plugin_name: staged.plugin.manifest.name,
            install_dir: staged.source_dir,
            already_installed: false,
            source: staged.source,
        });
    }

    let install_dir = plugin_dir_config.dir.join(&staged.plugin.manifest.name);
    copy_dir_recursive(&staged.source_dir, &install_dir)?;
    persist_plugin_enabled(
        site_root,
        &staged.plugin.manifest.name,
        plugin_dir_config.configured_value.as_deref(),
    )?;

    Ok(PluginInstallOutcome {
        plugin_name: staged.plugin.manifest.name,
        install_dir,
        already_installed: false,
        source: staged.source,
    })
}

pub fn uninstall_plugin(site_root: &Path, plugin_ref: &str) -> SiteResult<PluginUninstallOutcome> {
    let plugin_dir_config = resolve_plugin_dir(site_root)?;
    if let Some(installed) = find_installed_plugin(&plugin_dir_config.dir, plugin_ref)? {
        let usage_files = find_plugin_usage_files(
            site_root,
            &installed.plugin,
            &installed.dir_name,
            &plugin_dir_config.dir,
        )?;

        persist_plugin_disabled(
            site_root,
            &installed.plugin.manifest.name,
            &installed.dir_name,
        )?;
        std::fs::remove_dir_all(&installed.plugin.dir)?;

        return Ok(PluginUninstallOutcome {
            plugin_name: installed.plugin.manifest.name,
            removed_dir: Some(installed.plugin.dir),
            disabled_only: false,
            usage_files,
        });
    }

    let bundled = find_bundled_plugin(plugin_ref)?.ok_or_else(|| SiteError::Plugin {
        plugin: plugin_ref.to_string(),
        message: format!(
            "Plugin '{}' is not available in '{}' or bundled ferrosite plugins.",
            plugin_ref,
            plugin_dir_config.dir.display()
        ),
    })?;

    let usage_files = find_plugin_usage_files(
        site_root,
        &bundled.plugin,
        &bundled.dir_name,
        &plugin_dir_config.dir,
    )?;

    persist_plugin_disabled(
        site_root,
        &bundled.plugin.manifest.name,
        &bundled.dir_name,
    )?;

    Ok(PluginUninstallOutcome {
        plugin_name: bundled.plugin.manifest.name,
        removed_dir: None,
        disabled_only: true,
        usage_files,
    })
}

struct StagedPlugin {
    plugin: Plugin,
    source: String,
    _temp_dir: Option<TempDir>,
    source_dir: PathBuf,
}

fn resolve_plugin_dir(site_root: &Path) -> SiteResult<PluginDirConfig> {
    let config = load_site_config_for_root(site_root)?;
    Ok(PluginDirConfig {
        dir: site_plugins_dir(site_root, config.plugins.plugins_dir.as_deref()),
        configured_value: config.plugins.plugins_dir,
    })
}

fn clone_plugin_repo(source: &str, destination: &Path) -> SiteResult<()> {
    let status = Command::new("git")
        .arg("clone")
        .arg(source)
        .arg(destination)
        .status()
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => SiteError::Build(
                "git is required to install plugins from repositories.".to_string(),
            ),
            _ => SiteError::from(err),
        })?;

    if !status.success() {
        return Err(SiteError::Build(format!(
            "'git clone {}' failed with status {}",
            source, status
        )));
    }

    Ok(())
}

fn is_git_source(source: &str) -> bool {
    source.starts_with("https://")
        || source.starts_with("ssh://")
        || source.starts_with("git@")
        || source.starts_with("file://")
        || source.ends_with(".git")
        || Path::new(source).join(".git").exists()
}

fn find_bundled_plugin_dir(name: &str) -> SiteResult<PathBuf> {
    let plugins_dir = bundled_plugins_dir();
    let mut available = Vec::new();

    if !plugins_dir.exists() {
        return Err(SiteError::Plugin {
            plugin: name.to_string(),
            message: "No bundled plugins are available.".to_string(),
        });
    }

    for entry in std::fs::read_dir(&plugins_dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }

        let plugin_name = entry.file_name().to_string_lossy().into_owned();
        available.push(plugin_name.clone());

        if plugin_name == name && entry.path().join("manifest.toml").exists() {
            return Ok(entry.path());
        }
    }

    available.sort();
    available.dedup();

    Err(SiteError::Plugin {
        plugin: name.to_string(),
        message: if available.is_empty() {
            "No bundled plugins are available.".to_string()
        } else {
            format!(
                "No bundled plugin named '{}'. Available bundled plugins: {}",
                name,
                available.join(", ")
            )
        },
    })
}

fn find_bundled_plugin(plugin_ref: &str) -> SiteResult<Option<InstalledPlugin>> {
    find_installed_plugin(&bundled_plugins_dir(), plugin_ref)
}

fn find_installed_plugin(
    plugins_dir: &Path,
    plugin_ref: &str,
) -> SiteResult<Option<InstalledPlugin>> {
    if !plugins_dir.exists() {
        return Ok(None);
    }

    for entry in std::fs::read_dir(plugins_dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }

        let dir_name = entry.file_name().to_string_lossy().into_owned();
        let plugin = Plugin::from_dir(&entry.path())?;

        if plugin.manifest.name == plugin_ref || dir_name == plugin_ref {
            return Ok(Some(InstalledPlugin { plugin, dir_name }));
        }
    }

    Ok(None)
}

fn find_plugin_usage_files(
    site_root: &Path,
    plugin: &Plugin,
    dir_name: &str,
    plugins_dir: &Path,
) -> SiteResult<Vec<PathBuf>> {
    let patterns = plugin_usage_patterns(plugin, dir_name);
    if patterns.is_empty() {
        return Ok(Vec::new());
    }

    let mut usage_files = BTreeSet::new();

    for entry in WalkDir::new(site_root)
        .into_iter()
        .filter_entry(|entry| !should_skip_path(entry.path(), plugins_dir))
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        if path.file_name() == Some(OsStr::new("ferrosite.toml")) {
            continue;
        }

        let Ok(contents) = std::fs::read_to_string(path) else {
            continue;
        };

        if patterns.iter().any(|pattern| contents.contains(pattern)) {
            usage_files.insert(path.strip_prefix(site_root)?.to_path_buf());
        }
    }

    Ok(usage_files.into_iter().collect())
}

fn plugin_usage_patterns(plugin: &Plugin, dir_name: &str) -> Vec<String> {
    let mut patterns = BTreeSet::new();

    patterns.insert(plugin.manifest.name.clone());
    patterns.insert(dir_name.to_string());
    patterns.insert(plugin.manifest.worker_route.clone());

    for slot in &plugin.manifest.slots {
        patterns.insert(slot.clone());
    }

    for tag in infer_component_tags(&plugin.component_source) {
        patterns.insert(tag);
    }

    patterns
        .into_iter()
        .filter(|pattern| !pattern.trim().is_empty())
        .collect()
}

fn infer_component_tags(source: &str) -> Vec<String> {
    let mut tags = BTreeSet::new();

    extract_js_call_strings(source, r#"pfusch(""#, '"', &mut tags);
    extract_js_call_strings(source, "pfusch('", '\'', &mut tags);
    extract_js_call_strings(source, r#"customElements.define(""#, '"', &mut tags);
    extract_js_call_strings(source, "customElements.define('", '\'', &mut tags);

    tags.into_iter().collect()
}

fn extract_js_call_strings(
    source: &str,
    marker: &str,
    closing_quote: char,
    tags: &mut BTreeSet<String>,
) {
    let mut rest = source;

    while let Some(index) = rest.find(marker) {
        let after_marker = &rest[index + marker.len()..];
        if let Some(end_index) = after_marker.find(closing_quote) {
            let candidate = after_marker[..end_index].trim();
            if !candidate.is_empty() {
                tags.insert(candidate.to_string());
            }
            rest = &after_marker[end_index + 1..];
        } else {
            break;
        }
    }
}

fn should_skip_path(path: &Path, plugins_dir: &Path) -> bool {
    path == plugins_dir
        || path.starts_with(plugins_dir)
        || path.file_name().is_some_and(|name| {
            matches!(
                name.to_str(),
                Some(".git" | "target" | "dist" | "node_modules")
            )
        })
}

fn persist_plugin_enabled(
    site_root: &Path,
    plugin_name: &str,
    configured_plugins_dir: Option<&str>,
) -> SiteResult<()> {
    update_plugin_config(site_root, |plugins| {
        if !plugins.contains_key("plugins_dir") {
            plugins.insert(
                "plugins_dir".into(),
                toml::Value::String(configured_plugins_dir.unwrap_or("plugins").to_string()),
            );
        }

        let enabled = plugins
            .entry("enabled")
            .or_insert_with(|| toml::Value::Array(Vec::new()));

        match enabled {
            toml::Value::Array(items) => {
                if !items.iter().any(|item| item.as_str() == Some(plugin_name)) {
                    items.push(toml::Value::String(plugin_name.to_string()));
                }
                Ok(())
            }
            _ => Err(SiteError::Config(
                "Expected 'plugins.enabled' in ferrosite.toml to be an array.".to_string(),
            )),
        }
    })
}

fn persist_plugin_disabled(site_root: &Path, plugin_name: &str, dir_name: &str) -> SiteResult<()> {
    update_plugin_config(site_root, |plugins| {
        if let Some(enabled) = plugins.get_mut("enabled") {
            match enabled {
                toml::Value::Array(items) => {
                    items.retain(|item| {
                        item.as_str()
                            .map(|value| value != plugin_name && value != dir_name)
                            .unwrap_or(true)
                    });
                    Ok(())
                }
                _ => Err(SiteError::Config(
                    "Expected 'plugins.enabled' in ferrosite.toml to be an array.".to_string(),
                )),
            }
        } else {
            Ok(())
        }
    })
}

fn update_plugin_config(
    site_root: &Path,
    mut mutate: impl FnMut(&mut toml::map::Map<String, toml::Value>) -> SiteResult<()>,
) -> SiteResult<()> {
    let config_path = site_root.join("ferrosite.toml");
    if !config_path.exists() {
        return Err(SiteError::Config(format!(
            "No ferrosite.toml found in '{}'. Run 'ferrosite new <name>' first or pass '--root <site-dir>'.",
            site_root.display()
        )));
    }

    let raw = std::fs::read_to_string(&config_path)?;
    let mut value: toml::Value = toml::from_str(&raw)?;
    let root_table = value.as_table_mut().ok_or_else(|| {
        SiteError::Config(format!(
            "'{}' must contain a top-level TOML table.",
            config_path.display()
        ))
    })?;

    let plugins = ensure_toml_table(root_table, "plugins")?;
    mutate(plugins)?;

    let rendered = toml::to_string_pretty(&value)?;
    std::fs::write(&config_path, rendered)?;
    Ok(())
}

fn ensure_toml_table<'a>(
    table: &'a mut toml::map::Map<String, toml::Value>,
    key: &str,
) -> SiteResult<&'a mut toml::map::Map<String, toml::Value>> {
    let value = table
        .entry(key.to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

    match value {
        toml::Value::Table(table) => Ok(table),
        _ => Err(SiteError::Config(format!(
            "Expected '{}' in ferrosite.toml to be a table.",
            key
        ))),
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> SiteResult<()> {
    std::fs::create_dir_all(dst)?;

    for entry in WalkDir::new(src).into_iter().filter_map(|entry| entry.ok()) {
        let path = entry.path();
        let relative = path.strip_prefix(src)?;
        let destination = dst.join(relative);

        if path.is_dir() {
            std::fs::create_dir_all(&destination)?;
        } else {
            std::fs::copy(path, &destination)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_site_config() -> &'static str {
        r#"[site]
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
"#
    }

    #[test]
    fn install_bundled_plugin_copies_files_and_enables_it() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        std::fs::write(root.join("ferrosite.toml"), minimal_site_config()).expect("config");

        let result = install_plugin(root, "contact-form").expect("plugin install");

        assert_eq!(result.plugin_name, "contact-form");
        assert!(!result.already_installed);
        assert!(!root.join("plugins/contact-form/manifest.toml").exists());
        assert!(result.install_dir.ends_with("plugins/contact-form"));

        let config = std::fs::read_to_string(root.join("ferrosite.toml")).expect("updated config");
        let value: toml::Value = toml::from_str(&config).expect("valid toml");
        assert_eq!(value["plugins"]["plugins_dir"].as_str(), Some("plugins"));
        assert_eq!(
            value["plugins"]["enabled"][0].as_str(),
            Some("contact-form")
        );
    }

    #[test]
    fn uninstall_plugin_removes_dir_and_reports_usage_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        std::fs::write(root.join("ferrosite.toml"), minimal_site_config()).expect("config");
        std::fs::create_dir_all(root.join("content")).expect("content dir");
        std::fs::write(
            root.join("content/about.md"),
            "slot = 'contact-form'\n<ferrosite-contact-form></ferrosite-contact-form>",
        )
        .expect("usage file");

        install_plugin(root, "contact-form").expect("plugin install");
        let result = uninstall_plugin(root, "contact-form").expect("plugin uninstall");

        assert_eq!(result.plugin_name, "contact-form");
        assert!(result.disabled_only);
        assert!(result.removed_dir.is_none());
        assert_eq!(result.usage_files, vec![PathBuf::from("content/about.md")]);

        let config = std::fs::read_to_string(root.join("ferrosite.toml")).expect("updated config");
        let value: toml::Value = toml::from_str(&config).expect("valid toml");
        assert!(value["plugins"]["enabled"]
            .as_array()
            .is_some_and(|items| items.is_empty()));
    }

    #[test]
    fn install_plugin_from_git_uses_manifest_name_for_destination() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        std::fs::write(root.join("ferrosite.toml"), minimal_site_config()).expect("config");

        let repo_root = temp.path().join("git-plugin-repo");
        std::fs::create_dir_all(&repo_root).expect("repo dir");
        std::fs::write(
            repo_root.join("manifest.toml"),
            r#"[plugin]
name = "custom-plugin"
version = "1.0.0"
author = "Example"
slots = ["contact-form"]
component_file = "component.js"
worker_file = "worker.js"
worker_route = "/api/custom"
"#,
        )
        .expect("manifest");
        std::fs::write(
            repo_root.join("component.js"),
            "pfusch('custom-widget', {});",
        )
        .expect("component");
        std::fs::write(repo_root.join("worker.js"), "export default {};").expect("worker");

        let init_status = Command::new("git")
            .arg("init")
            .arg("-b")
            .arg("main")
            .current_dir(&repo_root)
            .status()
            .expect("git init");
        assert!(init_status.success());

        let email_status = Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&repo_root)
            .status()
            .expect("git config email");
        assert!(email_status.success());

        let name_status = Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_root)
            .status()
            .expect("git config name");
        assert!(name_status.success());

        let add_status = Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_root)
            .status()
            .expect("git add");
        assert!(add_status.success());

        let commit_status = Command::new("git")
            .args(["commit", "-m", "Initial plugin"])
            .current_dir(&repo_root)
            .status()
            .expect("git commit");
        assert!(commit_status.success());

        let result =
            install_plugin(root, repo_root.to_str().expect("repo path")).expect("git install");

        assert_eq!(result.plugin_name, "custom-plugin");
        assert!(root.join("plugins/custom-plugin/manifest.toml").exists());
        assert!(!root.join("plugins/git-plugin-repo").exists());

        let config = std::fs::read_to_string(root.join("ferrosite.toml")).expect("updated config");
        let value: toml::Value = toml::from_str(&config).expect("valid toml");
        assert_eq!(
            value["plugins"]["enabled"][0].as_str(),
            Some("custom-plugin")
        );
    }
}
