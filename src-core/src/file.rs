use std::{path::PathBuf, sync::OnceLock};

use directories::BaseDirs;

pub static ROOT_DIR: OnceLock<AppRoot> = OnceLock::new();

pub const SETTINGS_FILE: &str = "memory.json";

#[derive(Debug)]
/// The root of the applications file system setup.
pub struct AppRoot(PathBuf);

impl std::ops::Deref for AppRoot {
    type Target = PathBuf;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AppRoot {
    pub fn from_path(config: &str) -> Option<AppRoot> {
        let mut conf = PathBuf::new();

        conf.push(config);

        if conf.exists() {
            return Some(AppRoot(conf));
        } else {
            return None;
        }
    }

    /// creates an app root at the default locations under this name.
    pub fn default(name: &str) -> Option<AppRoot> {
        if name.starts_with(r"/") || name.starts_with(r"\") {
            log::warn!("Starting the file name with a file separator sets absolute path");
        }

        let proj = BaseDirs::new()?;
        Some(AppRoot(proj.config_local_dir().join(name)))
    }
}

pub enum Directory {
    BhapticsCache,
    Logs,
    Maps,
    Security,
    Sidecars,
}

pub fn resolve_dir(folder: Directory) -> PathBuf {
    let root = ROOT_DIR.get().expect("root directory hasn't been set yet");
    let root = root.0.clone();
    match folder {
        Directory::BhapticsCache => root.join("data"),
        Directory::Logs => root.join("logs"),
        Directory::Maps => root.join("map_configs"),
        Directory::Security => root.join("security"),
        Directory::Sidecars => root.join("sidecars"),
    }
}