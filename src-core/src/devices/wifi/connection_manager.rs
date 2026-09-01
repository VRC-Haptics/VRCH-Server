use rosc::{OscMessage, OscPacket, OscType};
use serde::Serialize;
use tokio::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Instant;
use tokio::sync::mpsc;

use crate::devices::wifi::config::WifiConfig;
use crate::devices::wifi::WifiTickSignal;
use crate::devices::ESP32Model;
use crate::osc::server::OscServer;
use crate::log_err;

/// Wraps a generic OscServer and messages the main device instance.
/// Kills itself when dropped.
#[derive(Serialize, Debug, Clone)]
pub struct WifiConnManager {
    pub server: OscServer,
}

impl WifiConnManager {
    pub async fn new(
        remote: SocketAddr,
        tx: mpsc::Sender<WifiTickSignal>,
    ) -> anyhow::Result<WifiConnManager> {
        // The closure that gets called anytime an osc message is recieved.
        let on_receive = move |msg: OscPacket| {
            packet(msg, &tx);
        };

        let local = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);

        let server = OscServer::new(local, Some(remote), on_receive).await?;
        Ok(WifiConnManager {
            server: server,
        })
    }

    pub async fn send(&self, data: &[u8]) -> io::Result<usize> {
        self.server.send(data).await
    }
}

fn packet(pkt: OscPacket, tx: &mpsc::Sender<WifiTickSignal>) {
    match pkt {
        OscPacket::Bundle(bdl) => {
            for msg in bdl.content {
                packet(msg, tx);
            }
        }
        OscPacket::Message(msg) => {
            message(msg, tx);
        }
    }
}

fn message(msg: OscMessage, tx: &mpsc::Sender<WifiTickSignal>) {
    //if heartbeat
    if msg.addr == "/hrtbt" {
        log_err!(tx.try_send(WifiTickSignal::NewHeartBeat(Instant::now())));

        // command was sent
    } else if msg.addr == "/command" {
        if let Some(OscType::String(cmd_str)) = msg.args.get(0) {
            // if confirmation that we reset something, invalidate config
            if cmd_str.contains("set to") {
                log::trace!("Recieved set to command: {:?}", cmd_str);
                log_err!(tx.try_send(WifiTickSignal::ResetConfig));
                return;
            }

            // if a response to our get-platform command
            if cmd_str.contains("PLATFORM") {
                log_err!(tx.try_send(WifiTickSignal::NewIdentifier(
                    ESP32Model::from_platform_string(&cmd_str),
                )));
                return;
            }

            match serde_json::from_str::<WifiConfig>(cmd_str) {
                Ok(command) => {
                    log_err!(tx.try_send(WifiTickSignal::NewConfig(Box::new(command))));
                    log::trace!("Found new device config");
                }
                Err(e) => {
                    log::error!(
                        "Failed to parse (needs to be fixed but idk)WifiCommand JSON: {}. Packet: {}",
                        e, cmd_str
                    );
                }
            }
        } else {
            log::error!("Non string type recieved from device");
        }
    } else if msg.addr == "/ping" {
        log_err!(tx.try_send(WifiTickSignal::PingConfirmation));
    } else if msg.addr == "/log" {
        if let Some(s) = msg.args.first().and_then(|arg| arg.clone().string()) {
            log_err!(tx.try_send(WifiTickSignal::NewDeviceLog(s)));
        }
    } else {
        log::error!(
            "Message with unknown address recieved: {}\tArgs: {:?}",
            msg.addr,
            msg.args
        );
    }
}

impl Drop for WifiConnManager {
    fn drop(&mut self) {
        self.server.stop();
    }
}
