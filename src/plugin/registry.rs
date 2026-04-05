use std::collections::HashMap;
use std::path::Path;

use crate::content::slot::SlotType;
use crate::error::{SiteError, SiteResult};
use crate::template::component::ComponentDef;

use super::Plugin;

/// All loaded plugins for a site build.
#[derive(Debug, Clone, Default)]
pub struct PluginRegistry {
    plugins: Vec<Plugin>,
}

impl PluginRegistry {
    pub fn load_from_dir(plugins_dir: &Path, enabled: &[String]) -> SiteResult<Self> {
        if !plugins_dir.exists() {
            return Ok(Self::default());
        }

        let plugins: Vec<SiteResult<(Plugin, String)>> = std::fs::read_dir(plugins_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|e| e.path().is_dir())
            .map(|entry| {
                let dir_name = entry.file_name().to_string_lossy().into_owned();
                Plugin::from_dir(&entry.path()).map(|plugin| (plugin, dir_name))
            })
            .collect();

        let mut loaded = Vec::new();
        let mut errors = Vec::new();

        for result in plugins {
            match result {
                Ok((plugin, dir_name)) => {
                    if enabled.is_empty()
                        || enabled.contains(&plugin.manifest.name)
                        || enabled.contains(&dir_name)
                    {
                        loaded.push(plugin);
                    }
                }
                Err(error) => errors.push(error),
            }
        }

        if !errors.is_empty() {
            eprintln!("Warning: {} plugin(s) failed to load:", errors.len());
            for error in &errors {
                eprintln!("  - {}", error);
            }
        }

        Ok(Self { plugins: loaded })
    }

    pub fn workers(&self) -> Vec<&Plugin> {
        self.plugins.iter().collect()
    }

    pub fn component_defs(&self) -> Vec<ComponentDef> {
        self.plugins
            .iter()
            .map(|plugin| plugin.to_component_def())
            .collect()
    }

    pub fn all_head_injections(&self) -> String {
        self.plugins
            .iter()
            .map(|plugin| plugin.render_head_injections())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn validate_no_conflicts(&self) -> SiteResult<()> {
        let mut occupied: HashMap<String, &str> = HashMap::new();
        for plugin in &self.plugins {
            for slot in &plugin.manifest.slots {
                if let Ok(slot_type) = slot.parse::<SlotType>() {
                    if !slot_type.is_multi() {
                        if occupied.contains_key(slot) {
                            return Err(SiteError::PluginSlotConflict {
                                plugin: plugin.manifest.name.clone(),
                                slot: slot.clone(),
                            });
                        }
                        occupied.insert(slot.clone(), &plugin.manifest.name);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_from_dir_matches_enabled_manifest_name() {
        let temp = tempfile::tempdir().expect("tempdir");
        let plugins_dir = temp.path().join("plugins");
        let plugin_dir = plugins_dir.join("repo-name");
        std::fs::create_dir_all(&plugin_dir).expect("plugin dir");
        std::fs::write(
            plugin_dir.join("manifest.toml"),
            r#"[plugin]
name = "manifest-name"
version = "1.0.0"
author = "Example"
slots = ["contact-form"]
component_file = "component.js"
worker_file = "worker.js"
worker_route = "/api/example"
"#,
        )
        .expect("manifest");
        std::fs::write(
            plugin_dir.join("component.js"),
            "pfusch('manifest-name', {});",
        )
        .expect("component");
        std::fs::write(plugin_dir.join("worker.js"), "export default {};").expect("worker");

        let registry = PluginRegistry::load_from_dir(&plugins_dir, &["manifest-name".to_string()])
            .expect("registry should load");

        assert_eq!(registry.len(), 1);
    }
}
