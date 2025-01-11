pub mod discovery;

//use rosc::OscType;
use serde::Serialize;

use crate::osc::server::OscServer;
use std::collections::HashMap;

use serde::Deserialize;

#[derive(serde::Serialize, Debug, Clone)]
pub struct  vrcInfo {
    osc_server: Option<OscServer>,
    in_port: Option<u16>,
    out_port: Option<u16>,
    avatar: Option<avatar>,
    raw_parameters: HashMap<String, OscType>,
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OscType { // rosc doesn't implement serde serialize
    Int(i32),
    Float(f32),
    String(String),
    Blob(Vec<u8>),
    Long(i64),
    Double(f64),
    Char(char),
    Bool(bool),
    Nil,
    Inf,
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