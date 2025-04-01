pub mod discovery;
pub mod parsing;

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
    pub vrc_connected: bool,
    pub osc_server: Option<OscServer>,
    pub query_server: Option<OscQueryServer>,
    pub in_port: Option<u16>,
    pub out_port: Option<u16>,
    pub avatar: Option<avatar>,
    pub haptics_prefix: String,
    pub menu_parameters: Arc<RwLock<Parameters>>,
    pub raw_parameters: Arc<RwLock<HashMap<String, Vec<OscType>>>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct avatar {
    avatar_id: String,
    menu_parameters: Option<Vec<String>>,
    haptic_parameters: Option<Vec<String>>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Parameters {
    pub parameters: HashMap<String, (String, f32)>, // k:name
}

impl Parameters {
    pub fn new() -> Parameters {
        let mut param = Parameters {
            parameters: HashMap::new(),
        };
        param.parameters.insert(
            "intensity".to_string(),
            ("/avatar/parameters/h_param/Intensity".to_string(), 1.),
        );
        param.parameters.insert(
            "offset".to_string(),
            ("/avatar/parameters/h_param/Offset".to_string(), 1.),
        );

        return param;
    }

    pub fn get(&self, param_name: &str) -> f32 {
        let value = self.parameters.get(param_name).unwrap().1;
        //println!("Got name:{}@value: {}", param_name, value);
        return value;
    }

    #[allow(dead_code)]
    pub fn addr(&self, param_name: &str) -> String {
        return self.parameters.get(param_name).unwrap().0.clone();
    }
}
