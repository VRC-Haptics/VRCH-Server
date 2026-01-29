use super::{Device, DeviceId, DeviceInfo, DeviceMessage};
use parking_lot::Mutex;
use std::{
    net::{SocketAddr, SocketAddrV4},
    sync::Arc,
    time::Instant,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::devices::{
    wifi::{config::WifiConfig, connection_manager::WifiConnManager},
    DeviceManager, ESP32Model,
};
use crate::mapping::haptic_node::HapticNode;
use crate::util::next_free_port;
use udp::broadcast::start_listen_broadcast;

mod config;
mod connection_manager;
mod udp;

pub fn start_wifi_devices(manager: &mut DeviceManager) {
    start_listen_broadcast(manager);
}

pub struct WifiDevice {
    name: String,
    mac: String,
    remote_addr: SocketAddr,
    cancel: CancellationToken,
    output: Vec<f32>,
    manager: mpsc::Sender<DeviceMessage>,
    live_state: Arc<Mutex<WifiDeviceState>>,
    connection: WifiConnManager,
}

pub struct WifiDeviceState {
    identifier: Option<ESP32Model>,
    logs: Vec<String>,
    nodes: Vec<HapticNode>,
    config: Option<WifiConfig>,
    last_heartbeat: Instant,
}

impl Default for WifiDeviceState {
    fn default() -> Self {
        WifiDeviceState {
            identifier: None,
            logs: vec![],
            nodes: vec![],
            config: None,
            last_heartbeat: Instant::now(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct WifiDeviceInfo {
    remote_addr: SocketAddr,
    name: String,
    mac: String,

    rssi: usize,
}

impl WifiDevice {
    /// NOTE: Needs to initialize everything
    pub async fn new(
        mac: String,
        ip: String,
        port: u16,
        name: String,
        tx: mpsc::Sender<DeviceMessage>,
    ) -> Option<WifiDevice> {
        let is_alive = CancellationToken::new();
        let ip = ip.parse().expect("Unable to parse ip");

        let recv_port = next_free_port(1500).unwrap();
        let (con_tx, mut rx) = mpsc::channel(5);
        let con = WifiConnManager::new(&recv_port, "/hrtbt".to_string(), con_tx).await;
        let state = Arc::new(Mutex::new(WifiDeviceState::default()));

        // processing messages from the connection manager
        let cancel_clone = is_alive.clone();
        let state_clone = Arc::clone(&state.clone());
        tokio::task::spawn(async move {
            loop {
                tokio::select! {
                    event = rx.recv() => {
                        let Some(event) = event else { break };

                        match event {
                            WifiTickSignal::NewDeviceLog(log) => {
                                let mut lock = state_clone.lock();
                                lock.logs.push(log);
                            }
                            WifiTickSignal::NewConfig(conf) => {
                                let mut lock = state_clone.lock();
                                lock.nodes = conf.node_map.clone();
                                lock.config = Some(conf)
                            }
                            WifiTickSignal::ResetConfig => {
                                let mut lock = state_clone.lock();
                                lock.config = None;
                            },
                            WifiTickSignal::NewIdentifier(ident) => {
                                let mut lock = state_clone.lock();
                                lock.identifier = Some(ident);
                            },
                            WifiTickSignal::NewHeartBeat(then) => {
                                let mut lock = state_clone.lock();
                                lock.last_heartbeat = then;
                            },
                            WifiTickSignal::PingConfirmation => return,
                        }
                    }

                    _ = cancel_clone.cancelled() => {
                        break;
                    }
                }
            }
        });

        start_tick(is_alive.clone(), Arc::clone(&state)).await;

        Some(WifiDevice {
            live_state: state,
            output: vec![],
            remote_addr: SocketAddr::V4(SocketAddrV4::new(ip, port)),
            name: name,
            mac: mac,
            manager: tx,
            cancel: is_alive,
            connection: con,
        })
    }

    fn get_info(&self) -> WifiDeviceInfo {
        WifiDeviceInfo {
            remote_addr: self.remote_addr.clone(),
            name: self.name.clone(),
            mac: self.mac.clone(),
            rssi: 0,
        }
    }

    pub fn reset_ping(&self) {}
}

async fn start_tick(cancel: CancellationToken, state: Arc<Mutex<WifiDeviceState>>) {

}

impl Device for WifiDevice {
    fn get_id(&self) -> DeviceId {
        DeviceId("this".to_string())
    }

    fn info(&self) -> DeviceInfo {
        DeviceInfo::Wifi(self.get_info())
    }

    fn disconnect(&mut self) {
        self.cancel.cancel();
    }
    async fn set_feedback(&mut self, values: &[f32]) {
        self.output = values.to_vec();
    }
    async fn set_manager_channel(&mut self, tx: mpsc::Sender<DeviceMessage>) {}
}

/// Set a signal that will be processed on the next device tick.
#[derive(Clone, Debug)]
pub enum WifiTickSignal {
    NewDeviceLog(String),
    NewConfig(WifiConfig),
    /// Should set wifi config to None, since we just changed it's value.
    ResetConfig,
    NewIdentifier(ESP32Model),
    NewHeartBeat(Instant),
    PingConfirmation,
}
