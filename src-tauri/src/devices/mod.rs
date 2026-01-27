//mod ble;
pub mod serial;
mod traits;
pub mod update;
pub mod wifi;
mod bhaptics;

use std::ops::Deref;
use std::sync::{Arc, Mutex, LazyLock};
use dashmap::DashMap;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use wifi::WifiDevice;
use bhaptics::BhapticBleDevice;

use crate::{devices::wifi::discovery::start_wifi_listener, mapping::{ haptic_node::HapticNode, interp::{GaussianState, InterpAlgo}}};
use crate::GlobalMap;

// Global device list; contains all active devices.
static DEVICES: LazyLock<DashMap<DeviceId, Device>> = LazyLock::new(||{DashMap::new()});

pub fn get_devices() -> &'static DashMap<DeviceId, Device> {
    &DEVICES
}

/// Starts all device handlers managing the various connected devices
pub async fn start_devices() {
    let register_fn = register_device;
    let remove_fn = remove_device;
    // calculates the 
    let gather_fn = get_intensity_from_nodes(&Vec<HapticNode>, );

    // Each listener is expected to handle removing adding, and pushing data to their devices.
    start_wifi_listener(app_handle);
}

pub fn register_device(dev: Device) -> Option<Device> {
    DEVICES.insert(dev.id.clone(), dev)
}

pub fn remove_device(id: DeviceId) -> Option<(DeviceId, Device)> {
    DEVICES.remove(&id)
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "variant", content = "value")]
pub enum DeviceType {
    Wifi(WifiDevice),
    BhapticBle(BhapticBleDevice),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DeviceId(pub String);

impl Deref for DeviceId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<String> for DeviceId {
    fn from(s: String) -> Self { Self(s) }
}

impl From<&str> for DeviceId {
    fn from(s: &str) -> Self { Self(s.to_owned()) }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
/// Represents a protocol-agnostic haptic device. 
/// 
/// A device is expected to handle it's own removal from the `DEVICES` list.
/// 
/// At its core a haptic device 
pub struct Device {
    /// ID garunteed to be unique to this device
    pub id: DeviceId,
    /// user-facing name
    pub name: String,
    /// Onput nodes attached to this device. 
    /// 
    /// NEVER change the length of this value without first updating the outputs.
    pub nodes: Arc<Mutex<OutputNodes>>,
    /// All factors that affect nodes on a device level
    pub factors: OutputFactors,
    /// Specific implementations .
    pub device_type: DeviceType,
} 

impl Device {
    /// Given a globalMap state, determine the output values for this device.
    /// 
    /// Used to udpate outputs to the most recent state.
    pub async fn collect_outputs(&mut self, map: &GlobalMap) {
        let mut nodes = self.nodes.lock().expect("Couldnt lock nodes.");
        let nodes_ref = nodes.nodes();
        let this = nodes.outputs_mut();
        map.get_intensity_from_haptic(nodes_ref, &self.factors.interp_algo, &true, this);
    }

    /// Removes this device from the static DEVICES list.
    pub fn remove(&self) {
        if remove_device(self.id).is_none() {
            log::error!("Unable to remove device with id: {} \nDevice: {}", id, self)
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OutputNodes {
    nodes: Vec<HapticNode>,
    outputs: Vec<f32>,
}

impl OutputNodes {
    pub fn set_nodes(&mut self, nodes: Vec<HapticNode>) {
        self.outputs.resize(nodes.len(), 0.0);
        self.nodes = nodes;
    }
    
    pub fn nodes(&self) -> &[HapticNode] {
        &self.nodes
    }
    
    pub fn outputs_mut(&mut self) -> &mut [f32] {
        &mut self.outputs
    }
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

impl Default for OutputFactors {
    fn default() -> Self {
        OutputFactors { sens_mult: 1.0, start_offset: 0.0, interp_algo: InterpAlgo::Gaussian(GaussianState::default()) }
    }
}

impl Device {
    /// Retrieves the ESP32's model
    pub fn get_esp_type(&self) -> ESP32Model {
        match &self.device_type {
            DeviceType::Wifi(d) => d.get_esp_type(),
            DeviceType::BhapticBle(_) => ESP32Model::Unknown,
        }
    }

    /// Starts a new wifi device with the given parameters
    pub async fn start_wifi_device(mac: String, ip: String, send_port: u16, name: String) -> Device {
        let wifi = WifiDevice::new(mac, ip, send_port, name);
        
        Device { id: DeviceId(wifi.mac.clone()), name, nodes: vec![], outputs: vec![], factors: OutputFactors::def, device_type: DeviceType::Wifi(wifi) }
    }

}


/// The firmware type returned from the device.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub enum ESP32Model {
    /// All original ESP32 variants
    ESP32,
    /// Standard ESP32-S2
    ESP32S2,
    /// ESP32-S2 with 16MB flash
    ESP32S2FH16,
    /// ESP32-S2 with 32MB flash  
    ESP32S2FH32,
    ESP32S3,
    ESP32C3,
    ESP32C2,
    ESP32C6,
    ESP8266,
    Unknown,
}

impl ESP32Model {
    pub fn ota_auth_port(&self) -> u16 {
        match *self {
            ESP32Model::ESP32 |
            ESP32Model::ESP32S2 |
            ESP32Model::ESP32C2 |
            ESP32Model::ESP32C3 |
            ESP32Model::ESP32C6 |
            ESP32Model::ESP32S2FH16 |
            ESP32Model::ESP32S2FH32 |
            ESP32Model::ESP32S3 =>  return 3232,
            ESP32Model::ESP8266 =>  return 8266,
            ESP32Model::Unknown =>  return 3232,
        }
    }
}

impl ESP32Model {
    /// Parse platform string from device (e.g., "PLATFORM ESP32-D0WDQ6")
    pub fn from_platform_string(platform: &str) -> Self {
        let model = platform
            .strip_prefix("PLATFORM ")
            .unwrap_or(platform)
            .trim();

        Self::from_model_string(model)
    }

    /// Parse raw model string (e.g., "ESP32-D0WDQ6")
    pub fn from_model_string(model: &str) -> Self {
        match model {
            // ESP8266
            "ESP8266" => Self::ESP8266,

            // ESP32 variants (all map to ESP32)
            s if s.starts_with("ESP32-D0WDQ6") => Self::ESP32,
            s if s.starts_with("ESP32-D0WD") => Self::ESP32,
            "ESP32-D2WD" => Self::ESP32,
            "ESP32-PICO-D2" => Self::ESP32,
            "ESP32-PICO-D4" => Self::ESP32,
            "ESP32-PICO-V3-02" => Self::ESP32,
            "ESP32-D0WDR2-V3" => Self::ESP32,

            // ESP32-S2 variants
            "ESP32-S2" => Self::ESP32S2,
            "ESP32-S2FH16" => Self::ESP32S2FH16,
            "ESP32-S2FH32" => Self::ESP32S2FH32,
            s if s.starts_with("ESP32-S2") => Self::ESP32S2, // Fallback for "ESP32-S2 (Unknown)"

            // Other models
            "ESP32-S3" => Self::ESP32S3,
            "ESP32-C3" => Self::ESP32C3,
            "ESP32-C2" => Self::ESP32C2,
            "ESP32-C6" => Self::ESP32C6,

            _ => Self::Unknown,
        }
    }

    /// Get display name for the model
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::ESP32 => "ESP32",
            Self::ESP32S2 => "ESP32-S2",
            Self::ESP32S2FH16 => "ESP32-S2 (16MB)",
            Self::ESP32S2FH32 => "ESP32-S2 (32MB)",
            Self::ESP32S3 => "ESP32-S3",
            Self::ESP32C3 => "ESP32-C3",
            Self::ESP32C2 => "ESP32-C2",
            Self::ESP32C6 => "ESP32-C6",
            Self::ESP8266 => "ESP8266",
            Self::Unknown => "Unknown",
        }
    }
}

impl serde::Serialize for ESP32Model {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_parsing() {
        assert_eq!(
            ESP32Model::from_platform_string("PLATFORM ESP8266"),
            ESP32Model::ESP8266
        );
        assert_eq!(
            ESP32Model::from_platform_string("PLATFORM ESP32-D0WDQ6-V3"),
            ESP32Model::ESP32
        );
        assert_eq!(
            ESP32Model::from_platform_string("PLATFORM ESP32-S2FH16"),
            ESP32Model::ESP32S2FH16
        );
        assert_eq!(
            ESP32Model::from_platform_string("PLATFORM ESP32-S3"),
            ESP32Model::ESP32S3
        );
        assert_eq!(
            ESP32Model::from_platform_string("PLATFORM Unknown"),
            ESP32Model::Unknown
        );
    }
}
