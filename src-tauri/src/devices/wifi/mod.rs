use super::{Device, DeviceId, DeviceInfo, DeviceMessage};
use parking_lot::Mutex;
use rosc::{encoder, OscMessage, OscPacket, OscType};
use std::{
    net::{SocketAddr, SocketAddrV4},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::devices::{
    wifi::{config::WifiConfig, connection_manager::WifiConnManager},
    DeviceManager, ESP32Model,
};
use crate::mapping::haptic_node::HapticNode;
use crate::util::next_free_port;
use udp::{broadcast::start_listen_broadcast, send_udp};

mod config;
mod connection_manager;
mod udp;

pub fn start_wifi_devices(manager: &mut DeviceManager) {
    udp::start_udp();
    start_listen_broadcast(manager);
}

pub struct WifiDevice {
    name: String,
    mac: String,
    remote_addr: SocketAddr,
    cancel: CancellationToken,
    manager: mpsc::Sender<DeviceMessage>,
    live_state: Arc<Mutex<WifiDeviceState>>,
    connection: WifiConnManager,
}

pub struct WifiDeviceState {
    output: Vec<f32>,
    push_map: bool,
    been_query: Option<Instant>,
    been_pinged: Option<Instant>,
    identifier: Option<ESP32Model>,
    logs: Vec<String>,
    nodes: Vec<HapticNode>,
    config: Option<WifiConfig>,
    last_heartbeat: Instant,
}

impl Default for WifiDeviceState {
    fn default() -> Self {
        WifiDeviceState {
            output: vec![],
            push_map: false,
            been_query: None,
            been_pinged: None,
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
        let tx_clone = tx.clone();
        let id_clone = DeviceId(mac.clone());
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
                                {
                                    let mut lock = state_clone.lock();
                                    lock.nodes = conf.node_map.clone();
                                    lock.config = Some(conf);
                                }
                                tx_clone.send(DeviceMessage::InfoDirty(id_clone.clone())).await;
                            }
                            WifiTickSignal::ResetConfig => {
                                {
                                let mut lock = state_clone.lock();
                                lock.config = None;
                                }
                                tx_clone.send(DeviceMessage::InfoDirty(id_clone.clone())).await;
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
                        // push our request to be removed upwards
                        send_kill(tx_clone.clone(), id_clone.clone());
                        break;
                    }
                }
            }
        });

        let addr = SocketAddr::V4(SocketAddrV4::new(ip, port));
        start_tick(
            is_alive.clone(),
            addr.clone(),
            Arc::clone(&state),
            recv_port,
        )
        .await;

        Some(WifiDevice {
            live_state: state,
            remote_addr: addr,
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

fn send_kill(tx: mpsc::Sender<DeviceMessage>, id: DeviceId) {
    tx.send(DeviceMessage::Remove(id));
}

async fn start_tick(
    cancel: CancellationToken,
    addr: SocketAddr,
    state: Arc<Mutex<WifiDeviceState>>,
    recieve_port: u16,
) {
    tokio::task::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    tick(&addr, &state, recieve_port, cancel.clone());
                }

                _ = cancel.cancelled() => {
                    break;
                }
            };
        }
    });
}

async fn tick(
    addr: &SocketAddr,
    state: &Arc<Mutex<WifiDeviceState>>,
    recieve_port: u16,
    cancel: CancellationToken,
) {
    let mut state = state.lock();

    // manager heartbeat/ping
    match state.been_pinged {
        Some(i) => {
            let since_pinged = state.last_heartbeat.saturating_duration_since(i);
            let diff = state.last_heartbeat.elapsed();
            let ttl = Duration::from_secs(3);
            // if; haven't recieved heartbeat, grace period for ping, or are canceled kill device.
            if diff > ttl && since_pinged > ttl || cancel.is_cancelled() {
                cancel.cancel();
                state.been_pinged;
                log::trace!("Killed device: {}", addr.clone());
            }
        }
        None => {
            let now = Instant::now();
            state.been_pinged = Some(now);
            log::info!("Pinging Board");
            let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
                addr: "/ping".to_string(),
                args: vec![OscType::Int(recieve_port.into())],
            }))
            .expect("Failed to build packet");
            send_udp(&msg_buf, &addr).await;
            return; // no need to spam the device multiple messages.
        }
    };

    // Should we update our node map in the WifiConfig
    let should_push = state.push_map && state.config.is_some();
    if should_push {
        state.push_map = false;
        let conf = state.config.as_ref().expect("Unable to asRef");
        send_udp(&build_set_map(&conf.node_map), &addr).await;
        return;
    };

    // query for our config if it isn't done already.
    if state.config.is_none() && state.been_query.is_none() {
        let last_query = state.been_query.unwrap();
        let since_query = Instant::now().duration_since(last_query);
        let ttl = Duration::from_secs(3);
        if since_query >= ttl {
            state.been_query = Some(Instant::now());
            let msg = encoder::encode(&OscPacket::Message(OscMessage {
                addr: "/command".to_string(),
                args: vec![OscType::String("get all".to_string())],
            }))
            .unwrap();
            send_udp(&msg, &addr).await;
            return;
        }
    }

    // compile driving message
    if let Some(conf) = &state.config {
        // need to account for our inputs vector not being same size as output nodes.
        let mut hex = String::new();
        let num_motors = conf.node_map.len();
        for mtr_idx in 0..num_motors {
            let num = state.output.get(mtr_idx).unwrap_or(&0.0); // if too small just fill zeros
            let clamped = num.clamp(0.0, 1.0);
            let scaled = (clamped * 0xffff as f32).round() as u16;
            hex.push_str(&format!("{:04x}", scaled));
        }

        let msg = rosc::OscMessage {
            addr: "/h".to_string(),
            args: vec![OscType::String(hex)],
        };
        let bytes = rosc::encoder::encode(&rosc::OscPacket::Message(msg)).unwrap();
        send_udp(&bytes, &addr).await;
        return;
    }
}

/// builds binary response to
fn build_set_map(map: &Vec<HapticNode>) -> Vec<u8> {
    let base = "SET NODE_MAP ".to_string();

    // Convert each HapticNode into its 8-byte hex representation.
    let hex_str: String = map
        .iter()
        .map(|node| {
            let bytes = node.to_bytes();
            // For each byte, produce a two-digit hex string.
            bytes
                .iter()
                .map(|byte| format!("{:02x}", byte))
                .collect::<String>()
        })
        .collect();
    let full = base + &hex_str;

    // compile to osc formatted packet
    let message = rosc::OscMessage {
        addr: "/command".to_string(),
        args: vec![OscType::String(full)],
    };
    let packet = rosc::OscPacket::Message(message);
    rosc::encoder::encode(&packet).unwrap()
}

impl Device for WifiDevice {
    fn get_id(&self) -> DeviceId {
        DeviceId(self.mac.clone())
    }

    fn info(&self) -> DeviceInfo {
        DeviceInfo::Wifi(self.get_info())
    }

    fn disconnect(&mut self) {
        self.cancel.cancel();
    }
    async fn set_feedback(&mut self, values: &[f32]) {
        let mut lock = self.live_state.lock();
        lock.output = values.to_vec();
    }
    async fn set_manager_channel(&mut self, tx: mpsc::Sender<DeviceMessage>) {
        self.manager = tx;
    }
}

/// Set a signal that will be processed on the next device tick.
#[derive(Clone, Debug)]
pub enum WifiTickSignal {
    /// reports from device
    NewDeviceLog(String),
    NewConfig(WifiConfig),
    /// Should set wifi config to None, since we just changed it's value.
    ResetConfig,
    NewIdentifier(ESP32Model),
    NewHeartBeat(Instant),
    PingConfirmation,
}
