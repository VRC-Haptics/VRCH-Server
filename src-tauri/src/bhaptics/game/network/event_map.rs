use std::collections::HashMap;

use base64::{engine::general_purpose, Engine};
use strum::EnumIter;

use crate::bhaptics::game::device_maps::{
    x40_vest::x40_vest_back, x40_vest::x40_vest_front, x6_head::x6_headset,
};
use crate::{mapping::Id, util::math::Vec3};

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
/// The base json message received via API.
pub struct BaseMessage {
    pub status: bool,
    pub message: GameMapping,
    pub error_message: Option<String>,
    pub timestamp: u64,
    pub code: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
/// The game mapping json request pulled from API.
pub struct GameMapping {
    pub id: String,               // non‑readable id
    pub create_time: u64,         // unix timestamp of creation
    pub name: String,             // user‑facing name
    pub creator: String,          // unreadable name
    pub workspace_id: String,     // unreadable
    pub version: i32,             // version number (can be negative)
    pub disable_validation: bool, // when connecting to the game websocket?
    pub haptic_mappings: Vec<HapticMapping>,
    pub category_options: Vec<String>, // list of designer‑set sorting methods
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HapticMapping {
    pub id: String,
    pub deploy_id: String,
    pub enable: bool,
    pub intensity: i32,
    pub key: String,
    /// One of the `GameMapping.category_options` values.
    pub category: String,
    pub description: String,
    pub update_time: u64, // last update timestamp
    pub tact_file_patterns: Vec<String>,
    pub audio_file_patterns: Vec<AudioFilePattern>,
    pub event_time: u32, // ms duration
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AudioFilePattern {
    pub pattern_id: String,  // empty
    pub snapshot_id: String, // empty
    pub position: String,
    pub clip: PatternClip,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PatternClip {
    pub id: String,
    pub name: String,
    pub version: i32,  // set to -1 by default
    pub duration: u32, // ms
    pub patterns: HashMap<PatternLocation, Vec<PatternLine>>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Eq, PartialEq, Hash, EnumIter)]
#[serde(rename_all = "PascalCase")]
pub enum PatternLocation {
    VestFront,
    VestBack,
    Head,
    ForearmL,
    ForearmR,
    #[serde(other)]
    Unknown,
}

impl PatternLocation {
    pub fn to_position(&self, index: usize) -> Vec3 {
        match self {
            PatternLocation::VestBack => x40_vest_back().rows[index],
            PatternLocation::VestFront => x40_vest_front().rows[index],
            PatternLocation::Head => x6_headset().rows[index],
            PatternLocation::Unknown => {
                log::error!("Unknown pattern location!");
                return Vec3::new(0., 0., 0.);
            }
            _ => {
                log::error!("Unimplemented pattern location {}!", self.to_input_tag());
                return Vec3::new(0., 0., 0.);
            }
        }
    }

    pub fn motor_count(&self) -> usize {
        match *self {
            Self::VestFront | Self::VestBack => 20,
            Self::Head => 6,
            Self::ForearmL | Self::ForearmR => 8,
            Self::Unknown => 0,
        }
    }

    /// returns the tag this location all input nodes belonging to this device share.
    pub fn to_input_tag(&self) -> &str {
        match *self {
            Self::VestFront => "Bhaptics_VestFront",
            Self::VestBack => "Bhaptics_VestBack",
            Self::Head => "Bhaptics_Headset",
            Self::ForearmL => "Bhaptics_ForearmL",
            Self::ForearmR => "Bhaptics_ForearmR",
            Self::Unknown => "Bhaptics_Unknown",
        }
    }

    /// gets the `InputNode` Id that is associated with this devices motor index.
    /// Returns None when the motor_index is out of range for this device.
    pub fn to_id(&self, motor_index: usize) -> Option<Id> {
        if motor_index >= self.motor_count() {
            return None;
        }

        let id = match *self {
            Self::VestFront => format!("Bhaptics_VestFront_{}", motor_index),
            Self::VestBack => format!("Bhaptics_VestBack_{}", motor_index),
            Self::Head => format!("Bhaptics_Head_{}", motor_index),
            Self::ForearmL => format!("Bhaptics_ForearmS_{}", motor_index),
            Self::ForearmR => format!("Bhaptics_ForearmR_{}", motor_index),
            Self::Unknown => format!("Bhaptics_Unknown_{}", motor_index),
        };

        return Some(Id(id));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternLine(pub Vec<u8>);

impl serde::Serialize for PatternLine {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let encoded = general_purpose::STANDARD.encode(&self.0);
        serializer.serialize_str(&encoded)
    }
}

impl<'de> serde::Deserialize<'de> for PatternLine {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        general_purpose::STANDARD
            .decode(&s)
            .map(PatternLine)
            .map_err(serde::de::Error::custom)
    }
}
