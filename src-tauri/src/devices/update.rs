use crate::devices::{wifi::ota, Device, DeviceType};
use std::{
    net::Ipv4Addr,
    ops::{Deref, DerefMut},
    str::FromStr,
};

/// Decides whether we have the capability of determining this devices eligibility.
pub fn is_updateable(dtype: DeviceType) -> bool {
    match dtype {
        DeviceType::Wifi(_) => true,
    }
}

/// Bundle containing all user-required information to start a firmware update.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Firmware {
    /// The ID that should be used to find the device:
    pub id: String,
    /// The method used to update firmware.
    pub method: UpdateMethod,
    /// Raw bytes of the .bin fw file.
    pub bytes: Vec<u8>,
}

impl Firmware {
    pub fn new(bytes: Vec<u8>, method: UpdateMethod, id: String) -> Self {
        Firmware {
            id: id,
            method: method,
            bytes: bytes,
        }
    }

    pub fn do_update(&self, dev: &Device) -> Result<(), String> {
        match &dev.device_type {
            DeviceType::Wifi(wifi) => match &self.method {
                UpdateMethod::OTA(pass) => {
                    let res = ota::update_ota(
                        self.bytes.clone(),
                        pass.0.clone(),
                        Ipv4Addr::from_str(&wifi.ip).expect("invalid ip"),
                    );
                    if res.is_none() {
                        return Err("Couldn't OTA update device".to_string());
                    }
                }
                _ => {
                    log::error!("Wrong device type");
                    return Err("Wrong Firmware type".to_string());
                }
            },
        }

        Ok(())
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct OtaPassword(String);
impl Deref for OtaPassword {
    type Target = String;

    fn deref(&self) -> &String {
        &self.0
    }
}

impl DerefMut for OtaPassword {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Which method to use.
///
/// Can contain information on details that aren't automatically negotiable.
#[derive(serde::Deserialize, serde::Serialize)]
pub enum UpdateMethod {
    /// over the air updatetyp. OtaPassword; authentication password (default: `Haptics-OTA`)
    OTA(OtaPassword),
    /// Not currently supported
    Serial(String),
}
