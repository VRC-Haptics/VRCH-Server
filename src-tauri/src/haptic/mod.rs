//mod ble;
mod traits;
pub mod wifi;

use tauri::AppHandle;
use wifi::WifiDevice;
use serde::{Deserialize, Serialize};

use crate::mapping::HapticMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "variant", content = "value")]
pub enum DeviceType {
    Wifi(WifiDevice),
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
    /// Contains the mapping parameters for this device
    pub map: HapticMap,
    /// Holds the varying fields/methods that need to be used for each type of device.
    pub device_type: DeviceType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Factors that affect or modulate output of Devices
pub struct OutputFactors {
    /// sensitivity multiplier (power limiter)
    pub sens_mult: f32,
    /// sensitivity set by user 
    pub user_sense: f32,
}


impl Device {
    /// Consumes wifi_device and creates a generic Device
    pub fn from_wifi(wifi_device: WifiDevice, app_handle: &AppHandle) -> Device {
        let mut new_device = Device { 
            id: wifi_device.mac.clone(), 
            name: wifi_device.name.clone(), 
            num_motors: 0, 
            is_alive: true, 
            factors: OutputFactors { sens_mult: 1.0, user_sense: 1.0 }, 
            map: HapticMap::new(0.3, 0.01), 
            device_type: DeviceType::Wifi(wifi_device), 
        };

        // Recall last saved sens_multiplier
        if let Some(old_offset) = crate::get_device_store_field(&app_handle, &new_device.id, "sens_mult") {
            new_device.factors.sens_mult = old_offset;
        }
        // Recall last user_sense
    
        return new_device;
    } 
}
