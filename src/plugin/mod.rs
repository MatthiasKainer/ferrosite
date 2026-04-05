mod loaded;
mod manager;
mod manifest;
mod registry;

pub use loaded::Plugin;
pub use manager::{install_plugin, uninstall_plugin, PluginInstallOutcome, PluginUninstallOutcome};
pub use manifest::{
    CommandDef, HeadInjection, HeadInjectionKind, PluginManifest, QueryDef, SandboxConfig,
};
pub use registry::PluginRegistry;
