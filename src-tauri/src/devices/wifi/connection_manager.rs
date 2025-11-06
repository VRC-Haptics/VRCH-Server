use rosc::{OscMessage, OscType};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, RwLock};
use std::time::SystemTime;
use std::{collections::HashMap, net::Ipv4Addr};

use crate::devices::ESP32Model;
use crate::devices::wifi::config::WifiConfig;
use crate::osc::server::OscServer;

/// handles the wifi device's connection. Sending, recieving, killing etc.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WifiConnManager {
    /// last SystemTime that we recieved a heartbeat
    pub last_hrtbt: Arc<Mutex<SystemTime>>,
    /// OSC address that will trigger the heartbeat.
    pub hrtbt_address: String,
    /// Port that WE recieve from the device on
    pub recv_port: u16,
    #[serde(skip)]
    server: Option<OscServer>,
    /// Holds the platform identifier
    pub identifier: Arc<RwLock<Option<ESP32Model>>>,
    /// Holds the last parsed command sent by the device.
    pub config: Arc<RwLock<Option<WifiConfig>>>,
}

impl WifiConnManager {
    pub fn new(recv_port: &u16, hrtbt_addr: String) -> WifiConnManager {
        let start_time = SystemTime::now();
        let last_hrtbt: Arc<Mutex<SystemTime>> = Arc::new(Mutex::new(start_time));
        let recieved_params: Arc<RwLock<HashMap<String, (Vec<OscType>, SystemTime)>>> =
            Arc::new(RwLock::new(HashMap::new()));
        let wifi_conf: Arc<RwLock<Option<WifiConfig>>> = Arc::new(RwLock::new(None));
        let ident: Arc<RwLock<Option<ESP32Model>>> = Arc::new(RwLock::new(None));

        let recieve_copy = recieved_params.clone();
        let last_hrtbt_cpy = last_hrtbt.clone();
        let hrtbt_addr_cpy = hrtbt_addr.clone();
        let wifi_conf_cpy = wifi_conf.clone();
        let ident_cpy = Arc::clone(&ident);

        // The closure that gets called anytime an osc message is recieved.
        let on_receive = move |msg: OscMessage| {
            let mut recieve_mut = recieve_copy.write().unwrap();
            recieve_mut.insert(msg.addr.clone(), (msg.args.clone(), SystemTime::now()));

            //if heartbeat
            if msg.addr == hrtbt_addr_cpy {
                let mut time_lock = last_hrtbt_cpy.lock().unwrap();
                *time_lock = SystemTime::now();

            // command was sent
            } else if msg.addr == "/command" {
                if let Some(OscType::String(cmd_str)) = msg.args.get(0) {
                    // if confirmation that we reset something, invalidate config
                    if cmd_str.contains(" set to ") {
                        log::trace!("Recieved set to command: {:?}", cmd_str);
                        let mut conf = wifi_conf_cpy.write().expect("Couldn't get write");
                        *conf = None;

                        return;
                    }

                    // if a response to our get-platform command
                    if cmd_str.contains("PLATFORM") {
                        let mut lock = ident_cpy.write().unwrap();
                        log::trace!("cmd_String: {}", cmd_str);
                        let plat = ESP32Model::from_platform_string(&cmd_str);
                        *lock = Some(plat.clone());
                        log::trace!("Set platform to: {plat:?}");

                        return;
                    }

                    match serde_json::from_str::<WifiConfig>(cmd_str) {
                        Ok(command) => {
                            //log::trace!("Set device config: {:?}", command);
                            let mut cmd_lock = wifi_conf_cpy.write().unwrap();
                            *cmd_lock = Some(command);
                        }
                        Err(e) => {
                            log::error!(
                                "Failed to parse (needs to be fixed but idk)WifiCommand JSON: {}. Packet: {}",
                                e, cmd_str
                            );
                        }
                    }
                }
            } else if msg.addr == "/ping" {
                log::trace!("Recieved ping with: {:?}", msg.args);
            } else {
                log::error!(
                    "Message with unknown address recieved: {}\tArgs: {:?}",
                    msg.addr,
                    msg.args
                );
            }
        };

        let mut server = OscServer::new(*recv_port, Ipv4Addr::UNSPECIFIED, on_receive);
        server.start();
        WifiConnManager {
            recv_port: recv_port.to_owned(),
            last_hrtbt: last_hrtbt,
            hrtbt_address: hrtbt_addr,
            server: Some(server),
            identifier: ident,
            config: wifi_conf,
        }
    }
}

impl Drop for WifiConnManager {
    fn drop(&mut self) {
        if let Some(ref mut server) = self.server {
            server.stop();
        }
    }
}
