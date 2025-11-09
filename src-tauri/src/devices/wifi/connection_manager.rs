use rosc::{OscMessage, OscType};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::sync::mpsc;
use std::time::SystemTime;

use crate::devices::wifi::config::WifiConfig;
use crate::devices::wifi::WifiTickSignal;
use crate::devices::ESP32Model;
use crate::osc::server::OscServer;

/// handles the wifi device's connection. Sending, Recieving, killing etc.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct WifiConnManager {
    /// Port that WE recieve from the device on
    pub recv_port: u16,
    #[serde(skip)]
    server: Option<OscServer>,
}

impl WifiConnManager {
    pub fn new(
        recv_port: &u16,
        hrtbt_addr: String,
        tx: mpsc::Sender<WifiTickSignal>,
    ) -> WifiConnManager {
        // The closure that gets called anytime an osc message is recieved.
        let on_receive = move |msg: OscMessage| {
            //if heartbeat
            if msg.addr == hrtbt_addr {
                tx.send(WifiTickSignal::NewHeartBeat(SystemTime::now()));

            // command was sent
            } else if msg.addr == "/command" {
                if let Some(OscType::String(cmd_str)) = msg.args.get(0) {
                    // if confirmation that we reset something, invalidate config
                    if cmd_str.contains(" set to ") {
                        log::trace!("Recieved set to command: {:?}", cmd_str);
                        tx.send(WifiTickSignal::ResetConfig);
                        return;
                    }

                    // if a response to our get-platform command
                    if cmd_str.contains("PLATFORM") {
                        tx.send(WifiTickSignal::NewIdentifier(
                            ESP32Model::from_platform_string(&cmd_str),
                        ));
                        return;
                    }

                    match serde_json::from_str::<WifiConfig>(cmd_str) {
                        Ok(command) => {
                            tx.send(WifiTickSignal::NewConfig(command));
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
                tx.send(WifiTickSignal::PingConfirmation);
            } else if msg.addr == "/log" {
                if let Some(s) = msg.args.first().and_then(|arg| arg.clone().string()) {
                    tx.send(WifiTickSignal::NewDeviceLog(s));
                }
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
            server: Some(server),
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
