use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use directories::BaseDirs;

pub static ROOT_DIR: OnceLock<AppRoot> = OnceLock::new();

/// Read-only directory that holds the files the installer shipped.
/// The host application sets this. Under Tauri use `app.path().resource_dir()`.
pub static RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

pub const SETTINGS_FILE: &str = "memory.json";

#[derive(Debug)]
/// The root of the writable part of the application file system.
///
/// On Windows and macOS both paths are the same directory.
/// On Linux `config` is `~/.config/<name>` and `data` is `~/.local/share/<name>`.
pub struct AppRoot {
    config: PathBuf,
    data: PathBuf,
}

impl std::ops::Deref for AppRoot {
    type Target = PathBuf;
    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

impl AppRoot {
    pub fn config_dir(&self) -> &Path {
        &self.config
    }

    pub fn data_dir(&self) -> &Path {
        &self.data
    }

    /// Uses one directory for config and for data. This keeps the old flat layout.
    pub fn from_path(config: &str) -> Option<AppRoot> {
        let conf = PathBuf::from(config);
        if let Err(e) = fs::create_dir_all(&conf) {
            log::error!("Failed to create app root {}: {e}", conf.display());
            return None;
        }
        Some(AppRoot {
            config: conf.clone(),
            data: conf,
        })
    }

    /// Creates an app root at the platform default locations under this name.
    pub fn default(name: &str) -> Option<AppRoot> {
        if name.starts_with('/') || name.starts_with('\\') {
            log::warn!("Starting the file name with a file separator sets absolute path");
        }

        let base = BaseDirs::new()?;
        let root = AppRoot {
            config: base.config_local_dir().join(name),
            data: base.data_local_dir().join(name),
        };

        for dir in [&root.config, &root.data] {
            if let Err(e) = fs::create_dir_all(dir) {
                log::error!("Failed to create app root {}: {e}", dir.display());
                return None;
            }
        }
        Some(root)
    }
}

#[derive(Debug, Clone, Copy)]
/// A writable directory that the application owns.
pub enum Directory {
    BhapticsCache,
    Logs,
    Maps,
    CamMaps,
    Security,
}

impl Directory {
    fn sub_path(self) -> &'static str {
        match self {
            Directory::BhapticsCache => "data",
            Directory::Logs => "logs",
            Directory::Maps => "map_configs",
            Directory::CamMaps => "cam_configs",
            Directory::Security => "security",
        }
    }
}

/// Returns a writable directory and creates it if it is absent.
pub fn resolve_dir(folder: Directory) -> PathBuf {
    let root = ROOT_DIR.get().expect("root directory hasn't been set yet");
    let path = root.data.join(folder.sub_path());

    if let Err(e) = fs::create_dir_all(&path) {
        log::error!("Failed to create {}: {e}", path.display());
    }
    path
}

/// Returns the path of a file that the installer shipped. Returns `None` if the file is absent.
pub fn resource(relative: &str) -> Option<PathBuf> {
    let path = RESOURCE_DIR.get()?.join(relative);
    path.exists().then_some(path)
}

/// Returns the path of a bundled native library.
///
/// Pass the file stem without the extension, for example `listen-for-vrc`.
/// The function adds `.dll` on Windows, `.so` on Linux, and `.dylib` on macOS.
pub fn native_lib(stem: &str) -> Option<PathBuf> {
    let file = format!("{stem}{}", std::env::consts::DLL_SUFFIX);
    let path = RESOURCE_DIR.get()?.join("sidecars").join(&file);

    if !path.exists() {
        log::error!("Native library is missing at {}", path.display());
        return None;
    }
    Some(path)
}

/// Returns the path of a bundled helper program.
///
/// Linux and macOS packages ship no sidecars, so this always returns `None` there.
pub fn sidecar(name: &str) -> Option<PathBuf> {
    if !cfg!(target_os = "windows") {
        return None;
    }
    resource(&format!("sidecars/{name}"))
}

/// Should be called once after base directory initialized
pub fn seed_from_resources() -> io::Result<()> {
    let Some(res) = RESOURCE_DIR.get() else {
        log::warn!("Resource directory is not set. Skipping the seed step.");
        return Ok(());
    };

    for folder in [Directory::Maps, Directory::Security] {
        let src = res.join(folder.sub_path());
        if !src.is_dir() {
            log::debug!("No bundled {} directory at {}", folder.sub_path(), src.display());
            continue;
        }
        copy_missing(&src, &resolve_dir(folder))?;
    }
    Ok(())
}

fn copy_missing(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());

        if entry.file_type()?.is_dir() {
            copy_missing(&entry.path(), &target)?;
        } else if !target.exists() {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
