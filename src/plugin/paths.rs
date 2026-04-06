use std::path::{Path, PathBuf};

pub fn bundled_plugins_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins")
}

pub fn site_plugins_dir(site_root: &Path, configured_plugins_dir: Option<&str>) -> PathBuf {
    configured_plugins_dir
        .map(|dir| site_root.join(dir))
        .unwrap_or_else(|| site_root.join("plugins"))
}

pub fn plugin_search_dirs(site_root: &Path, configured_plugins_dir: Option<&str>) -> Vec<PathBuf> {
    let mut dirs = vec![
        site_plugins_dir(site_root, configured_plugins_dir),
        bundled_plugins_dir(),
    ];
    dirs.sort();
    dirs.dedup();
    dirs
}
