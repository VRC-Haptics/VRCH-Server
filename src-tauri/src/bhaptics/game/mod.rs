/// A mess of serialization crap that sorta works to deserialize the weirdly formatted AuthenticationInit Message
pub mod network;
mod v1;
mod v2;
mod v3;
mod player_messages;

use super::game::v3::BhapticsApiV3;
use crate::{
    bhaptics::game::{v1::BhapticsApiV1, v2::BhapticsApiV2}, 
    mapping::{
        event::Event, 
        global_map::GlobalMap
    }
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
    // handle to V3 api
    v3_api: Arc<Mutex<BhapticsApiV3>>,
    /// handle to v2 api
    v2_api: Arc<Mutex<BhapticsApiV2>>,
    /// handle to v1 api
    v1_api: Arc<Mutex<BhapticsApiV1>>,
    // The Global instance of the global map, jsut for backreferencing
    global_map: Arc<Mutex<GlobalMap>>,
}

impl BhapticsGame {
    /// Performs startup for all the api versions
    pub fn new(global_map: Arc<Mutex<GlobalMap>>) -> Arc<Mutex<Self>> {
        let v3 = BhapticsApiV3::new(global_map.clone());
        let v2 = BhapticsApiV2::new();
        let v1 = BhapticsApiV1::new();

        let game = Arc::new(Mutex::new(BhapticsGame {
            active_api: None,
            v3_api: v3,
            v2_api: v2,
            v1_api: v1,
            global_map: Arc::clone(&global_map),
        }));

        game
    }

    /// Shuts down all Bhaptics API's
    pub fn shutdown(&self) {
        self.v3_api.lock().expect("couldn't lock v3 api").shutdown();
    }

    /// updates the GlobalMap with values from the game since last tick
    pub fn tick(&mut self) -> Vec<Event> {
        let mut new_events = Vec::new();

        let mut v3_lock = self.v3_api.lock().expect("Unable to lock v3");
        let mut v2_lock = self.v2_api.lock().expect("Unable to lock v2");
        let mut v1_lock = self.v1_api.lock().expect("Unable to lock v1");
        new_events.append(&mut v3_lock.tick());
        new_events.append(&mut v2_lock.tick());
        new_events.append(&mut v1_lock.tick());

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
