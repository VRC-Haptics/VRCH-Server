use directories::ProjectDirs;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        LazyLock,
    },
    time::Duration,
};

static CONFIG: LazyLock<RwLock<Config>> =
    LazyLock::new(|| RwLock::new(load_config().unwrap_or_default()));
static DIRTY: AtomicBool = AtomicBool::new(false);

pub async fn start_config(save_delay: Duration) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(save_delay);
            if DIRTY.swap(false, Ordering::Relaxed) {
                save_config(&CONFIG.read());
            }
        }
    });
}

/// Usage: `let device_settings = state::get(|c| {c.saved_devices.get(id);`
pub fn get<T>(f: impl FnOnce(&Config) -> T) -> T {
    f(&CONFIG.read())
}

/// Some overhead with atomic bool, disk writes are buffered.
pub fn set(f: impl FnOnce(&mut Config)) {
    let mut config = CONFIG.write();
    f(&mut config);
    DIRTY.store(true, Ordering::Relaxed);
}

/// Helper to set persistant store values
pub fn set_device_field(id: &str, f: impl FnOnce(&mut PerDevice)) {
    let mut config = CONFIG.write();
    f(config.saved_devices.entry(id.to_owned()).or_default());
    DIRTY.store(true, Ordering::Relaxed);
}

/// Helper to get persistant store values
pub fn get_device<T>(id: &str, f: impl FnOnce(&PerDevice) -> T) -> Option<T> {
    CONFIG.read().saved_devices.get(id).map(f)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub wifi_device_timeout: f32,
    pub saved_devices: HashMap<String, PerDevice>,
    pub vrc_settings: VrcSettings,
}

impl Default for Config {
    fn default() -> Self {
        Self { 
            wifi_device_timeout: 3.0, 
            saved_devices: HashMap::new(), 
            vrc_settings: VrcSettings::default()
        }
    }
}

impl Default for VrcSettings {
    fn default() -> Self {
        Self { 
            velocity_ratio: 0.5,
        }
    }
}

impl Default for PerDevice {
    fn default() -> Self {
        Self { 
            intensity: None,
            offset: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VrcSettings {
    pub velocity_ratio:f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerDevice {
    pub intensity: Option<f32>,
    pub offset: Option<f32>,
} 

fn config_path() -> PathBuf {
    ProjectDirs::from("com", "vrch", "app")
        .expect("no valid config directory")
        .config_dir()
        .join("memory.json")
}

fn load_config() -> Option<Config> {
    let data = fs::read_to_string(config_path()).ok()?;
    serde_json::from_str(&data).ok()
}

fn save_config(config: &Config) {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, serde_json::to_string_pretty(config).unwrap());
}
