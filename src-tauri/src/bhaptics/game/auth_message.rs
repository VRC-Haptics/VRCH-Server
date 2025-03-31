use crate::util::deserialization::skip_outer_quotes;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
/// Collects alot of weird classes to handle serialization and deserialization of the AuthInit Message
pub struct AuthInitMessage {
    #[serde(deserialize_with = "skip_outer_quotes")]
    pub authentication: AuthenticationSection,
    #[serde(deserialize_with = "skip_outer_quotes")]
    pub haptic: HapticSection,
}

impl AuthInitMessage {
    pub fn from_message_str(raw: &str) -> Result<AuthInitMessage, Box<serde_json::Error>> {
        let res: AuthInitMessage = serde_json::from_str(raw)?;
        return Ok(res);
    }
}


#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AuthenticationSection {
    #[serde(rename = "cipher")]
    pub cipher: String,
    #[serde(rename = "applicationId")]
    pub application_id: String,
    #[serde(rename = "nonceHashValue")]
    pub nonce_hash_value: String,
    #[serde(rename = "applicationIdHashValue")]
    pub application_id_hash_value: String,
    #[serde(rename = "sdkApiKey")]
    pub sdk_api_key: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct HapticSection {
    pub status: bool,
    pub message: HapticSectionMessage,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct HapticSectionMessage {
    pub id: String,
    #[serde(rename = "createTime")]
    pub create_time: u64,
    pub name: String,
    pub creator: String,
    #[serde(rename = "workspaceId")]
    pub workspace_id: String,
    pub version: u32,
    #[serde(rename = "disableValidation")]
    pub disable_validation: bool,
    #[serde(rename = "hapticMappings")]
    pub haptic_mappings: Vec<HapticMapping>,
    #[serde(rename = "categoryOptions")]
    pub category_options: Vec<String>,
    pub description: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct HapticMapping {
    pub enable: bool,
    pub intensity: i32,
    pub key: String,
    pub category: String,
    pub description: String,
    #[serde(rename = "updateTime")]
    pub update_time: i64,
    #[serde(rename = "tactFilePatterns")]
    pub tact_file_patterns: Vec<TactFilePattern>,
    #[serde(rename = "eventTime")]
    pub event_time: i32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TactFilePattern {
    pub position: String,
    #[serde(rename = "tactFile")]
    pub tact_file: TactFile,
}


#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TactFile {
    pub name: String,
    pub tracks: Vec<Track>,
    pub layout: Layout,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Track {
    pub enable: bool,
    pub effects: Vec<Effect>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Effect {
    pub name: String,
    #[serde(rename = "offsetTime")]
    pub offset_time: i32,
    #[serde(rename = "startTime")]
    pub start_time: i32,
    pub modes: serde_json::Value, // TODO: Actually impelement modes
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum Modes {
    VestFront(Mode),
    VestBack(Mode),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Mode {
    #[serde(rename = "dotMode")]
    pub dot_mode: DotMode,
    #[serde(rename = "pathMode")]
    pub path_mode: PathMode,
    pub mode: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DotMode {
    #[serde(rename = "dotConnected")]
    pub dot_connected: bool,
    pub feedback: Vec<DotFeedback>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct DotFeedback {
    #[serde(rename = "startTime")]
    pub start_time: i32,
    #[serde(rename = "endTime")]
    pub end_time: i32,
    #[serde(rename = "playbackType")]
    pub playback_type: String,
    #[serde(rename = "pointList")]
    pub point_list: Vec<Point>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PathMode {
    pub feedback: Vec<PathFeedback>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PathFeedback {
    #[serde(rename = "movingPattern")]
    pub moving_pattern: String,
    #[serde(rename = "playbackType")]
    pub playback_type: String,
    pub visible: bool,
    #[serde(rename = "pointList")]
    pub point_list: Vec<Point>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Point {
    pub intensity: f64,
    pub time: i32,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Layout {
    pub name: String,
    #[serde(rename = "type")]
    pub type_field: String,
    pub layouts: serde_json::Value,
}