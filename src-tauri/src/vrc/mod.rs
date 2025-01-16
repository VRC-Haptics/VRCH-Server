pub mod discovery;

use discovery::OscQueryServer;
use serde::{ Serialize, Deserialize };
use rosc::OscType;
use crate::osc::server::OscServer;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

#[derive(serde::Serialize, Debug, Clone)]
pub struct  VrcInfo {
    pub osc_server: Option<OscServer>,
    pub query_server: Option<OscQueryServer>,
    pub in_port: Option<u16>,
    pub out_port: Option<u16>,
    pub avatar: Option<avatar>,
    pub raw_parameters: Arc<RwLock<HashMap<String, Vec<OscType>>>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct avatar {
    avatar_id: String,
    menu_parameters: Option<Vec<String>>,
    haptic_parameters: Option<Vec<String>>,
}

/* what is imported: 
#[derive(serde::Serialize, Clone)]
pub struct OscServer {
    port: u16,
    address: Ipv4Addr,
    #[serde(skip)]
    close_handle: Option<mpsc::Sender<()>>,
    #[serde(skip)]
    on_receive: Arc<Mutex<dyn Fn(OscMessage) + Send + Sync>>,
}
 */