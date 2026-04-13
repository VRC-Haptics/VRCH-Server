use crate::util::deserialization::skip_outer_quotes;

pub struct ParsedAuth {
    pub name: String,
    pub application_id: String,
    pub api_key: String,
    pub creator_id: String,
    pub workspace_id: String,
}

pub fn parse_auth_init(contents: &str) -> Result<ParsedAuth, Box<dyn std::error::Error>> {
    // The raw message has double-escaped backslashes from the SDK.
    let cleaned = contents.replace(r"\\", "");
    let msg: AuthInitMessage = serde_json::from_str(&cleaned)?;

    Ok(ParsedAuth {
        name: msg.haptic.message.name,
        application_id: msg.authentication.application_id,
        api_key: msg.authentication.sdk_api_key,
        creator_id: msg.haptic.message.creator,
        workspace_id: msg.haptic.message.workspace_id,
    })
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct AuthInitMessage {
    #[serde(deserialize_with = "skip_outer_quotes")]
    authentication: AuthenticationSection,
    #[serde(deserialize_with = "skip_outer_quotes")]
    haptic: HapticSection,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct AuthenticationSection {
    #[serde(rename = "cipher")]
    cipher: String,
    #[serde(rename = "applicationId")]
    application_id: String,
    #[serde(rename = "nonceHashValue")]
    nonce_hash_value: String,
    #[serde(rename = "applicationIdHashValue")]
    application_id_hash_value: String,
    #[serde(rename = "sdkApiKey")]
    sdk_api_key: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct HapticSection {
    status: bool,
    message: HapticSectionMessage,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct HapticSectionMessage {
    id: String,
    #[serde(rename = "createTime")]
    create_time: u64,
    name: String,
    creator: String,
    #[serde(rename = "workspaceId")]
    workspace_id: String,
    version: u32,
    #[serde(rename = "disableValidation")]
    disable_validation: bool,
    #[serde(rename = "hapticMappings")]
    haptic_mappings: Vec<HapticMapping>,
    #[serde(rename = "categoryOptions")]
    category_options: Vec<String>,
    description: String,
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
    pub modes: serde_json::Value,
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