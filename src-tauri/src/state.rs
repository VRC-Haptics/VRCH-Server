use arc_swap::ArcSwap;
use dashmap::DashMap;
use directories::ProjectDirs;
use hazarc::{AtomicArc, Cache};
use serde::{Deserialize, Serialize};
use std::{
    sync::Mutex,
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock,
    },
    time::Duration,
};

use crate::{
    devices::DeviceId, log_err, mapping::interp::{GaussianState, InterpAlgo}
};

// not intended to be accessed publicly. Use functions below
static PER_DEVICE: LazyLock<Mutex<Vec<Arc<ArcSwap<PerDevice>>>>> = LazyLock::new(|| {
    let mut list = Vec::new();
    let cfg = CONFIG.load();
    cfg.saved_devices.iter().map(|d| {
        list.push(ArcSwap::new(Arc::new(d.clone())));
    });

    Mutex::new(list)
});
static CONFIG: LazyLock<AtomicArc<Config>> =
    LazyLock::new(|| AtomicArc::new(load_config().unwrap_or_default().into()));
static DIRTY: AtomicBool = AtomicBool::new(false);

/// starts persisting our config to disk. Spawns new task.
pub async fn start_config_save(save_delay: Duration) {
    let _ = CONFIG.load(); // make sure it is intialized early.'
    log_err!(PER_DEVICE.lock()); // make sure we can load devices.

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(save_delay).await;
            if DIRTY.swap(false, Ordering::Relaxed) {
                // TODO: Push saved devices to our CONFIG field.
                save_config();
            }
        }
    });
}

/// retrieves a clone of the given field.
///
/// This value is owned and doesn't update atomically.
pub fn clone_field<F, T>(f: F) -> T
where
    F: FnOnce(&Config) -> &T,
    T: Clone,
{
    f(&CONFIG.load()).clone()
}

/// Used to cheaply access an atomic value later without blocking.
/// This function itself is non-trivial compute wise.
///
/// # USAGE:
///
/// ```
/// let mut cache = cache();
/// loop {
///     let cfg = cache.load(); // only a few pointer calls
///     log::trace!("{cfg.mapping_menu}");
/// }
///
/// ```
///
/// NOTE: It is still not super cheap to write to this value.
pub fn cache() -> Cache<AtomicArc<Config>> {
    Cache::new(*CONFIG)
}

/// Runs function F on the config to produce changes. THIS CLONES THE WHOLE CONFIG, BE AWARE OF THIS.
///
/// These changes are immediately available to all references using the cache,
/// but are stored to drive at specified intervals
pub fn update(f: impl FnOnce(&mut Config)) {
    let mut new = Config::clone(&CONFIG.load());
    f(&mut new);
    CONFIG.store(new.into());
    DIRTY.store(true, Ordering::Relaxed);
}

/// Swaps the current config for the given one.
///
/// This is non-trivial and should be considered as such.
pub fn swap(conf: Config) {
    CONFIG.store(conf.into());
    DIRTY.store(true, Ordering::Relaxed)
}

pub fn get_device_conf(id: &DeviceId) -> ArcSwap<PerDevice> { 
    let Ok(mut lock) = PER_DEVICE.lock() else {
        log::error!("Unable to lock device settings");
        panic!("Lock Error");
    };

    let Some(this) = lock
        .iter()
        .find(|d| d.load().id == *id) else {
            let Some(existing) = lock.iter().find(|d| d.load().id == id ) else {
                let new = ArcSwap::new(Arc::new(PerDevice::default(id)));
                lock.push(new.);
                return;
            };

            existing.store(Arc::new(PerDevice::default(id)));
        };


}

/// Handles device setting device fields, initializes to default values if none found.
///
/// Changes are immediately visible to all references.
pub fn update_device_field(id: &DeviceId, f: impl FnOnce(&mut PerDevice)) {
    let mut config = Config::clone(&CONFIG.load());

    // Find existing index or push new and get that index
    let idx = config
        .saved_devices
        .iter()
        .position(|d| d.id == *id)
        .unwrap_or_else(|| {
            log::trace!("default device config for: {id:?}");
            config.saved_devices.push(PerDevice::default(id.clone()));
            config.saved_devices.len() - 1
        });

    f(&mut config.saved_devices[idx]);
    CONFIG.store(config.into());
    DIRTY.store(true, Ordering::Relaxed);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub wifi_device_timeout: f32,
    pub mapping_menu: StandardMenu,
    /// Stale data, intended for persisting to disk.
    ///
    /// see `PER_DEVICE` for live data.
    saved_devices: Vec<PerDevice>,
    pub vrc_settings: VrcSettings,
}

/// The common factors that will be used across all devices to modify output.
/// Game inputs should insert values that will be used in device calculations here.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct StandardMenu {
    pub intensity: f32, // multiplier set by user in-game
    pub enable: bool,   // Flat enable or disable all haptics
}

impl Default for Config {
    fn default() -> Self {
        Self {
            wifi_device_timeout: 3.0,
            mapping_menu: StandardMenu::default(),
            saved_devices: vec![],
            vrc_settings: VrcSettings::default(),
        }
    }
}

impl Default for StandardMenu {
    fn default() -> Self {
        StandardMenu {
            intensity: 1.0,
            enable: true,
        }
    }
}

impl PerDevice {
    fn default(id: DeviceId) -> Self {
        Self {
            id: id,
            intensity: 1.0,
            offset: 0.01,
            interp_algo: InterpAlgo::Gaussian(GaussianState::default()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Persistant state related to vrc specifically.
pub struct VrcSettings {
    /// How much weight distance has, 1-`dist_weight` = the velocity weight
    pub velocity_ratio: f32,
    /// the magic velocity multiplier. 1 is reasonable, if fast.
    pub velocity_mult: f32,
    /// Number of value entries to keep track of for velocity measurements.
    ///
    /// VRC Refreshes at 10hz max, so 10*seconds should work just fine.
    /// Will only refresh on program restart.
    pub sample_cache: usize,

    /// Takes an average of all data recieved within this past time frame.
    ///
    /// Smooths motor acceleration.
    pub smoothing_time: Duration,
}

impl Default for VrcSettings {
    fn default() -> Self {
        Self {
            velocity_ratio: 0.5,
            velocity_mult: 1.0,
            sample_cache: 10,
            smoothing_time: Duration::from_secs_f32(0.12),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Settings that contain all information about this specific device (Specified by device id)
pub struct PerDevice {
    pub id: DeviceId,
    pub intensity: f32,
    pub offset: f32,
    pub interp_algo: InterpAlgo,
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

fn save_config() {
    let config = CONFIG.load();
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, serde_json::to_string_pretty(&config).unwrap());
}
