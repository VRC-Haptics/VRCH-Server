//mod ble;
mod traits;
pub mod wifi;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use wifi::WifiDevice;

use crate::mapping::interp::{GaussianState, InterpAlgo};
use crate::GlobalMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "variant", content = "value")]
pub enum DeviceType {
    Wifi(WifiDevice),
}

impl DeviceType {
    fn tick(&mut self, is_alive: &mut bool, factors: &mut OutputFactors, inputs: &GlobalMap) {
        match self {
            DeviceType::Wifi(dev) => {
                dev.tick(is_alive, factors, inputs);
            }
            _ => log::error!("unknown device type"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Device {
    /// ID garunteed to be unique to this device
    pub id: String,
    /// user-facing name
    pub name: String,
    /// number of motors this device controls
    pub num_motors: u32,
    /// Not garunteed to change on death, but device should be removed if false
    pub is_alive: bool,
    /// Factors that are used in the modulation of devices
    pub factors: OutputFactors,
    /// Holds the varying fields/methods that need to be used for each type of device.
    pub device_type: DeviceType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Factors that affect or modulate output of Devices
pub struct OutputFactors {
    /// sensitivity multiplier (power limiter)
    pub sens_mult: f32,
    /// the lowest value that produces feedback
    pub start_offset: f32,
    /// Interpolation algorithm
    pub interp_algo: InterpAlgo,
}

impl Device {
    /// Consumes wifi_device and creates a generic Device (with the wifiDevice as a child)
    pub fn from_wifi(wifi_device: WifiDevice, app_handle: &AppHandle) -> Device {
        let init_interp = GaussianState::new(0.002, 0.10, 0.1);

        let mut new_device = Device {
            id: wifi_device.mac.clone(),
            name: wifi_device.name.clone(),
            num_motors: 0,
            is_alive: true,
            factors: OutputFactors {
                sens_mult: 1.0,
                start_offset: 0.0,
                interp_algo: InterpAlgo::Gaussian(init_interp),
            },
            device_type: DeviceType::Wifi(wifi_device),
        };

        // Recall last saved sens_multiplier
        if let Some(old_offset) =
            crate::get_device_store_field(&app_handle, &new_device.id, "sens_mult")
        {
            new_device.factors.sens_mult = old_offset;
        }

        return new_device;
    }
}
