pub mod discovery;

use crate::osc::server::OscServer;
use discovery::OscQueryServer;
use rosc::OscType;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

#[derive(serde::Serialize, Debug, Clone)]
pub struct VrcInfo {
    pub osc_server: Option<OscServer>,
    pub query_server: Option<OscQueryServer>,
    pub in_port: Option<u16>,
    pub out_port: Option<u16>,
    pub avatar: Option<avatar>,
    pub haptics_prefix: String,
    pub raw_parameters: Arc<RwLock<HashMap<String, Vec<OscType>>>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct avatar {
    avatar_id: String,
    menu_parameters: Option<Vec<String>>,
    haptic_parameters: Option<Vec<String>>,
}

pub struct Parameters {
    strength: f32,
}
