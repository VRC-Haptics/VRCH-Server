use std::collections::HashMap;

use base64::{engine::general_purpose, Engine};

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
    pub id: String,                    // non‑readable id
    pub create_time: u64,              // unix timestamp of creation
    pub name: String,                  // user‑facing name
    pub creator: String,               // unreadable name
    pub workspace_id: String,          // unreadable
    pub version: i32,                  // version number (can be negative)
    pub disable_validation: bool,      // when connecting to the game websocket?
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
    pub update_time: u64,                      // last update timestamp
    pub tact_file_patterns: Vec<String>,
    pub audio_file_patterns: Vec<AudioFilePattern>,
    pub event_time: u32, // ms duration
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AudioFilePattern {
    pub pattern_id: String, // empty
    pub snapshot_id: String, // empty
    pub position: String,
    pub clip: PatternClip,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PatternClip {
    pub id: String,
    pub name: String,
    pub version: i32,   // set to -1 by default
    pub duration: u32,  // ms
    pub patterns: HashMap<PatternLocation, Vec<PatternLine>>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Eq, PartialEq, Hash)]
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
