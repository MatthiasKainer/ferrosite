mod loaded;
mod manager;
mod manifest;
mod paths;
mod registry;

pub use loaded::Plugin;
pub use manager::{install_plugin, uninstall_plugin, PluginInstallOutcome, PluginUninstallOutcome};
pub use manifest::{
    CommandDef, HeadInjection, HeadInjectionKind, PluginManifest, QueryDef, SandboxConfig,
};
pub use paths::{bundled_plugins_dir, plugin_search_dirs, site_plugins_dir};
pub use registry::PluginRegistry;
