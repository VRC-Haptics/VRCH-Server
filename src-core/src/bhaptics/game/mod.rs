/// A mess of serialization crap that sorta works to deserialize the weirdly formatted AuthenticationInit Message
pub mod network;
mod v3;
mod v2;

use crate::mapping::{MapHandle, event::Event};

use network::event_map::PatternLocation;
use serde;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

pub enum ApiVersion {
    V3,
    V2,
    V1,
}

pub struct ActiveApi {
    pub game_name: String,
    pub api_version: ApiVersion,
    pub sdk_version: u32,
}

#[derive(Debug, Clone)]
pub struct BhapticHandle {
    pub shutdown_token: CancellationToken,
}

impl BhapticHandle {
    pub fn shutdown(&self) {
        self.shutdown_token.cancel();
    }
}

/// Starts all bhaptics api versions and returns the user handle
pub async fn start_bhaptics(map: MapHandle) -> BhapticHandle {
    let token = CancellationToken::new();

    tokio::spawn(v3::run_server(map.clone(), token.child_token()));
    tokio::spawn(v2::run_server(map.clone(), token.child_token()));
    // tokio::spawn(v1::run_server(map.clone(), token.child_token()));

    BhapticHandle { shutdown_token: token }

}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "message")]
/// Intermediary enum to direct string parsing
enum RecievedMessage {
    /// The first message sent from the game
    SdkRequestAuthInit(String),
    /// The message that triggers the start of a haptic event
    SdkPlay(String),
    /// Clears all active events
    SdkStopAll,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "Type", content = "message")]
pub enum SendMessage {
    ServerReady,
    ServerEventNameList(Vec<String>),
    ServerEventList(Vec<ServerEvent>),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerEvent {
    event_name: String,
    event_time: u32,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SdkPlayMessage {
    event_name: String,
    request_id: u32,
    position: u32,
    intensity: f32,
    duration: f32,
    offset_angle_x: f32,
    offset_y: f32,
}
