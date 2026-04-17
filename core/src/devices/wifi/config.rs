use crate::mapping::haptic_node::HapticNode;
use serde::de::Error as SerdeError;
use serde::{Deserialize, Deserializer};
use std::convert::TryInto;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
/// Wifi Device config struct
pub struct WifiConfig {
    pub wifi_ssid: String,
    pub wifi_password: String,
    pub mdns_name: String,
    #[serde(deserialize_with = "deserialize_from_str")]
    pub node_map: Vec<HapticNode>,
    pub i2c_scl: u32,
    pub i2c_sda: u32,
    pub i2c_speed: u32,
    pub motor_map_i2c_num: u32,
    pub motor_map_i2c: Vec<u32>,
    pub motor_map_ledc_num: u32,
    pub motor_map_ledc: Vec<u32>,
    pub config_version: u32,
}

/// Takes a string and converts it into a Vec<HapticNode>.
/// I HATE THE WAY THIS IS DONE, but it's good enough for now
pub fn deserialize_from_str<'de, D>(deserializer: D) -> Result<Vec<HapticNode>, D::Error>
where
    D: Deserializer<'de>,
{
    // First, deserialize the field as a String.
    let s = String::deserialize(deserializer)?;

    // Ensure the hex string has an even length.
    if s.len() % 2 != 0 {
        return Err(D::Error::custom("Hex string has an odd length"));
    }

    // Convert the hex string to a Vec<u8>
    let mut bytes = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        let byte_str = &s[i..i + 2];
        let byte = u8::from_str_radix(byte_str, 16)
            .map_err(|_| D::Error::custom("Invalid hex value in node_map"))?;
        bytes.push(byte);
    }

    // Check that the byte array length is a multiple of 8.
    if bytes.len() % 8 != 0 {
        return Err(D::Error::custom(
            "Invalid node_map length: must be a multiple of 8 bytes",
        ));
    }

    // Process the bytes in 8-byte chunks to create HapticNode instances.
    let nodes = bytes
        .chunks_exact(8)
        .map(|chunk| {
            let arr: [u8; 8] = chunk
                .try_into()
                .map_err(|_| D::Error::custom("Chunk conversion failed"))?;
            Ok(HapticNode::from_bytes(arr))
        })
        .collect::<Result<Vec<_>, D::Error>>()?;

    Ok(nodes)
}
