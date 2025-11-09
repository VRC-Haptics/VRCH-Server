//mod ble;
pub mod serial;
mod traits;
pub mod update;
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
    /// Retrieves the ESP32's model
    pub fn get_esp_type(&self) -> ESP32Model {
        match &self.device_type {
            DeviceType::Wifi(d) => d.get_esp_type(),
        }
    }

    /// Consumes wifi_device and creates a generic Device (with the wifiDevice as a child)
    pub fn from_wifi(wifi_device: WifiDevice, app_handle: &AppHandle) -> Device {
        let init_interp = GaussianState::new(0.002, 0.05);

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
