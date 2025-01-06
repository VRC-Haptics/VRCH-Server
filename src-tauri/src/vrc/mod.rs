pub mod discovery;

use rosc::OscType;

use crate::osc::server::OscServer;
use std::collections::HashMap;

pub struct  vrcInfo {
    osc_server: Option<OscServer>,
    in_port: Option<u16>,
    out_port: Option<u16>,
    avatar: Option<avatar>,
    raw_parameters: HashMap<String, OscType>,
}

pub struct avatar {
    avatar_id: String,
    menu_parameters: Option<Vec<String>>,
    haptic_parameters: Option<Vec<String>>,
}