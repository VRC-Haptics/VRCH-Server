use arc_swap::ArcSwap;
use boxcar::Vec as AppendVec;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, LazyLock, Mutex},
    time::Duration,
};
use tokio::sync::mpsc::{channel, Receiver, Sender};

use crate::{
    devices::DeviceId,
    log_err,
    mapping::interp::{GaussianState, InterpAlgo},
};

// not intended to be accessed publicly. Use functions below
static CONFIG: LazyLock<Config> = LazyLock::new(|| load_config().unwrap_or_default().into());
pub static UPDATE_TX: LazyLock<Sender<UpdateEvent>> = LazyLock::new(|| {
    let (tx, rx) = channel(10);
    *UPDATE_RX.lock().unwrap() = Some(rx);
    tx
});

/// Only intended to be used by tauri frontend
pub static UPDATE_RX: Mutex<Option<Receiver<UpdateEvent>>> = Mutex::new(None);
static DIRTY: AtomicBool = AtomicBool::new(false);


/// Heavy function, persists a snapshot of our config to the disk.
pub fn save_config() {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, serde_json::to_string_pretty(&*CONFIG).unwrap());
}

/// An event marks a change **THAT HAS ALREADY BEEN COMPLETED**
///
/// Completely encapsulates every way the app state can change.
/// In cases where a field in a config struct field needs to be referenced, a closure is provided that returns the value.
///
/// Intended to be used to emit events to the ui. Probably paired with a macro like send_update!().
pub enum UpdateEvent {
    /// Possibility of only being interested in one device.
    DeviceSettings(DeviceId),
    /// Device settings are cheap enough to just rebuild every time
    GeneralDeviceSettings,
}

/// returns bare static reference to global app configuration (state)
pub fn get_config() -> &'static Config {
    &*CONFIG
}

/// Main method for retrieving a read-only view of a device configuration.
///
/// Returns;
/// Saved device index,
/// static reference to device.
pub fn get_device(id: &DeviceId) -> (usize, &'static ArcSwap<PerDevice>) {
    let Some(existing) = CONFIG
        .devices
        .states
        .iter()
        .find(|(_, d)| d.load().id == *id)
    else {
        let _ = update_device(Arc::new(PerDevice::default(id.clone())));
        return CONFIG
            .devices
            .states
            .iter()
            .find(|(_, d)| d.load().id == *id)
            .expect("device still not in saved_devices");
    };
    existing
}

/// Either updates an existing PerDevice with the same id, or adds it to known devices
/// returns index device was stored at.
pub fn update_device(state: Arc<PerDevice>) -> usize {
    let Some((idx, existing)) = CONFIG
        .devices
        .states
        .iter()
        .find(|(_, d)| d.load().id == state.id)
    else {
        return CONFIG.devices.states.push(ArcSwap::new(state));
    };
    existing.swap(state);
    idx
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
/// Hub for individual app states
///
/// This is never intended to be moved at runtime and references to the children are of static lifetime.
pub struct Config {
    pub mapping_menu: ArcSwap<StandardMenu>,
    pub devices: Devices,
    pub vrc_settings: ArcSwap<VrcSettings>,
}

/// Handles all app state underneath the Device Manager
#[derive(Debug)]
pub struct Devices {
    pub wifi_device_timeout: ArcSwap<f32>,
    /// Inner ArcSwap allows for device settings to be updated, without changing static lifetime.
    pub states: AppendVec<ArcSwap<PerDevice>>,
}

/// The common factors that will be used across all devices to modify output.
/// Game inputs should insert values that will be used in device calculations here.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct StandardMenu {
    /// multiplier set by user in-game
    pub intensity: f32,
    /// Flat enable or disable all haptics
    pub enable: bool,
}

impl serde::Serialize for Devices {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let Devices {
            wifi_device_timeout,
            states,
        } = self;

        #[derive(Serialize)]
        struct Proxy {
            pub wifi_device_timeout: f32,
            pub states: Vec<PerDevice>,
        }

        Proxy {
            wifi_device_timeout: wifi_device_timeout.load_full().as_ref().clone(),
            states: states
                .iter()
                .map(|(_, d)| d.load_full().as_ref().clone())
                .collect(),
        }
        .serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for Devices {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Proxy {
            pub wifi_device_timeout: f32,
            pub states: Vec<PerDevice>,
        }

        let Proxy {
            wifi_device_timeout,
            states,
        } = Proxy::deserialize(deserializer)?;

        let arc_states = AppendVec::new();
        for state in states {
            arc_states.push(ArcSwap::new(Arc::new(state)));
        }

        Ok(Devices {
            wifi_device_timeout: ArcSwap::new(Arc::new(wifi_device_timeout)),
            states: arc_states,
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            devices: Devices {
                wifi_device_timeout: ArcSwap::new(Arc::new(3.0)),
                states: AppendVec::new(),
            },
            mapping_menu: ArcSwap::new(Arc::new(StandardMenu::default())),
            vrc_settings: ArcSwap::new(Arc::new(VrcSettings::default())),
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
