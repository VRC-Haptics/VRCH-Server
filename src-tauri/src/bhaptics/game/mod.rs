/// A mess of serialization crap that sorta works to deserialize the weirdly formatted AuthenticationInit Message
pub mod network;
mod player_messages;

use crate::{
    mapping::event::Event,
};

use network::event_map::PatternLocation;
use serde;
use std::sync::{Arc, Mutex};

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

/// Master Struct containing a unified interface for various api versions
pub struct BhapticsGame {
    // active connection
    pub active_api: Option<ActiveApi>,
}

impl BhapticsGame {
    /// Performs startup for all the api versions
    pub fn new() -> Arc<Mutex<Self>> {


        let game = Arc::new(Mutex::new(BhapticsGame {
            active_api: None,
        }));

        game
    }

    /// Shuts down all Bhaptics API's
    pub fn shutdown(&self) {
    }

    /// updates the GlobalMap with values from the game since last tick
    pub fn tick(&mut self) -> Vec<Event> {
        let mut new_events = Vec::new();

        new_events
    }
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
