use std::str::FromStr;
use std::{collections::HashMap, net::Ipv4Addr };
use std::sync::{ Arc, Mutex, RwLock };
use serde::{ Deserialize, Serialize };
use rosc::{ OscMessage, OscType };
use std::time::SystemTime;

use crate::osc::server::OscServer;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeviceConnManager {
    pub last_hrtbt: Arc<Mutex<SystemTime>>,
    pub hrtbt_address: String,
    pub recv_port: u16,
    #[serde(skip)]
    server: Option<OscServer>,
}

impl DeviceConnManager {
    pub fn new(port: u16, hrtbt_addr: String) -> DeviceConnManager {
        let start_time = SystemTime::now();
        let last_hrtbt: Arc<Mutex<SystemTime>>= Arc::new(Mutex::new(start_time));
        let recieved_params: Arc<RwLock<HashMap<String, (Vec<OscType>, SystemTime)>>> = Arc::new(RwLock::new(HashMap::new()));
        
        let recieve_copy = recieved_params.clone();
        let last_hrtbt_cpy = last_hrtbt.clone();
        let hrtbt_addr_cpy = hrtbt_addr.clone();
        let on_receive = move |msg: OscMessage| {
            let mut recieve_mut = recieve_copy.write().unwrap();
            recieve_mut.insert(msg.addr.clone(), (msg.args, SystemTime::now()));

            //if heartbeat
            if msg.addr == hrtbt_addr_cpy {
                let mut time_lock = last_hrtbt_cpy.lock().unwrap();
                *time_lock = SystemTime::now();
            }
        };

        let mut server = OscServer::new(port, Ipv4Addr::from_str("0.0.0.0").unwrap(), on_receive);
        server.start();
        DeviceConnManager {
            last_hrtbt: last_hrtbt,
            recv_port: port,
            hrtbt_address: hrtbt_addr,
            server:Some(server),
        }
    }
}