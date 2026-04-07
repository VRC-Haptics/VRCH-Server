use super::{Device, DeviceId, DeviceInfo, DeviceMessage};
use parking_lot::{Mutex, RwLock};
use rosc::{encoder, OscMessage, OscPacket, OscType};
use std::{
    net::{SocketAddr, SocketAddrV4},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{devices::{
    DeviceHandle, ESP32Model, wifi::{config::WifiConfig, connection_manager::WifiConnManager}
}, log_err, state::{self, PerDevice}};
use crate::mapping::haptic_node::HapticNode;
use crate::util::next_free_port;
use udp::{broadcast::start_listen_broadcast, send_udp};

mod config;
mod connection_manager;
mod udp;

pub async fn start_wifi_devices(manager: &mut DeviceHandle) {
    log::trace!("Starting wifi devices");
    udp::start_udp().await;
    start_listen_broadcast(manager).await;
}

#[derive(Debug)]
pub struct WifiDevice {
    name: String,
    mac: String,
    remote_addr: SocketAddr,
    cancel: CancellationToken,
    manager: mpsc::Sender<DeviceMessage>,
    live_state: Arc<Mutex<WifiDeviceState>>,
    connection: WifiConnManager,
}

#[derive(Debug)]
pub struct WifiDeviceState {
    output: Arc<RwLock<Vec<f32>>>,
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
            output: Arc::new(RwLock::new(vec![])),
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

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, specta::Type)]
pub struct WifiDeviceInfo {
    pub nodes: Vec<HapticNode>,
    pub remote_addr: SocketAddr,
    pub name: String,
    pub mac: String,
    pub rssi: usize,
    pub esp_model: ESP32Model,
    pub offset: f32,
    pub intensity: f32,
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
                                log::trace!("New log: {:?}", log);
                                let mut lock = state_clone.lock();
                                lock.logs.push(log);
                            }
                            WifiTickSignal::NewConfig(conf) => {
                                {
                                    let mut lock = state_clone.lock();
                                    lock.nodes = conf.node_map.clone();
                                    lock.config = Some(*conf);
                                    // resize buffer if needed
                                    let mut out = lock.output.write();
                                    if out.len() != lock.nodes.len() {
                                        *out = vec![0.0; lock.nodes.len()];
                                    }
                                }
                                let _ = tx_clone.send(DeviceMessage::InfoDirty(id_clone.clone())).await;
                            }
                            WifiTickSignal::ResetConfig => {
                                log::trace!("Reset config");
                                {
                                let mut lock = state_clone.lock();
                                lock.config = None;
                                }
                                let _ = tx_clone.send(DeviceMessage::InfoDirty(id_clone.clone())).await;
                            },
                            WifiTickSignal::NewIdentifier(ident) => {
                                log::trace!("recieved ident");
                                let mut lock = state_clone.lock();
                                lock.identifier = Some(ident);
                            },
                            WifiTickSignal::NewHeartBeat(then) => {
                                let mut lock = state_clone.lock();
                                lock.last_heartbeat = then;
                            },
                            WifiTickSignal::PingConfirmation => {log::trace!("got ping confirmation: {id_clone:?}")},
                        }
                    }

                    _ = cancel_clone.cancelled() => {
                        // push our request to be removed upwards
                        let _ = tx_clone.send(DeviceMessage::Remove(id_clone)).await;
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

    /// Please be mindful this call causes lockign with internal state,
    /// 
    fn get_info(&self) -> WifiDeviceInfo {
        let state = self.live_state.lock();
        WifiDeviceInfo {
            nodes: state.nodes.clone(),
            remote_addr: self.remote_addr.clone(),
            name: self.name.clone(),
            mac: self.mac.clone(),
            rssi: 0,
            esp_model: ESP32Model::Unknown,
            intensity: 1.0,
            offset: 0.0,
        }
    }

    pub fn reset_ping(&self) {}
}

async fn start_tick(
    cancel: CancellationToken,
    addr: SocketAddr,
    state: Arc<Mutex<WifiDeviceState>>,
    recieve_port: u16,
) {
    tokio::task::spawn(async move {
        let period = Duration::from_millis(20);
        let mut interval = tokio::time::interval(period);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let start = Instant::now();
                    tick(&addr, &state, recieve_port, cancel.clone()).await;
                    let elapsed = start.elapsed();
                    if elapsed > period {
                        log::warn!("Device tick overran: {:?} (limit {:?})", elapsed, period);
                    }
                }

                _ = cancel.cancelled() => {
                    break;
                }
            };
        }
    });
}

enum TickAction {
    Kill,
    Ping(Vec<u8>),
    PushMap(Vec<u8>),
    Query(Vec<u8>),
    Drive(Vec<u8>),
    None,
}

async fn tick(
    addr: &SocketAddr,
    state: &Arc<Mutex<WifiDeviceState>>,
    recieve_port: u16,
    cancel: CancellationToken,
) {
    let action = {
        let mut state = state.lock();

        match state.been_pinged {
            Some(i) => {
                let since_pinged = Instant::now().duration_since(i);
                let diff = state.last_heartbeat.elapsed();
                let ttl = Duration::from_secs(3);
                if diff > ttl && since_pinged > ttl || cancel.is_cancelled() {
                    cancel.cancel();
                    TickAction::Kill
                } else {
                    // fall through to other checks below
                    TickAction::None
                }
            }
            None => {
                state.been_pinged = Some(Instant::now());
                let msg_buf = encoder::encode(&OscPacket::Message(OscMessage {
                    addr: "/ping".to_string(),
                    args: vec![OscType::Int(recieve_port.into())],
                }))
                .expect("Failed to build packet");
                TickAction::Ping(msg_buf)
            }
        }
    };

    // Handle actions that need no further lock
    match action {
        TickAction::Kill => {
            cancel.cancel()
        },
        TickAction::Ping(buf) => {
            log::trace!("Sent ping: {}", addr);
            let _ = send_udp(&buf, addr).await;
            return;
        },
        TickAction::None |
        TickAction::Drive(_) |
        TickAction::PushMap(_) |
        TickAction::Query(_) => {}
    }

    // Second lock scope for remaining checks
    let action = {
        let mut state = state.lock();

        if state.push_map && state.config.is_some() {
            state.push_map = false;
            let conf = state.config.as_ref().unwrap();
            log::trace!("Pushing config to device: {addr:?}");
            TickAction::PushMap(build_set_map(&conf.node_map))
        } else if state.config.is_none() && state.been_query.is_none() {
            state.been_query = Some(Instant::now());
            let msg = encoder::encode(&OscPacket::Message(OscMessage {
                addr: "/command".to_string(),
                args: vec![OscType::String("get all".to_string())],
            })).unwrap();
            log::trace!("Query Device: {addr:?}");
            TickAction::Query(msg)
        } else if let Some(conf) = &state.config {
            let mut hex = String::new();
            for mtr_idx in 0..conf.node_map.len() {
                let output = state.output.read();
                let num = output.get(mtr_idx).unwrap_or(&0.0);
                let scaled = (num.clamp(0.0, 1.0) * 0xffff as f32).round() as u16;
                hex.push_str(&format!("{:04x}", scaled));
            }
            let bytes = rosc::encoder::encode(&rosc::OscPacket::Message(rosc::OscMessage {
                addr: "/h".to_string(),
                args: vec![OscType::String(hex)],
            })).unwrap();
            TickAction::Drive(bytes)
        }else {
            TickAction::None
        }
    }; // lock dropped

    match action {
        TickAction::PushMap(buf) => {log_err!(send_udp(&buf, addr).await)},
        TickAction::Query(buf) => {log_err!(send_udp(&buf, addr).await)},
        TickAction::Drive(buf) => {log_err!(send_udp(&buf, addr).await)},
        TickAction::Ping(buf ) => {
            log::trace!("Port: {:?}", addr.port());
            let mut new = addr.clone();
            new.set_port(1027);
            log_err!(send_udp(&buf, &new).await)
        },
        TickAction::Kill |
        TickAction::None => {},
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

    /// Please note this causes internal locking and while minor,
    /// should be limited where possible
    fn info(&self) -> DeviceInfo {
        let (_, cfg) = state::get_device(&DeviceId(self.mac.clone()));
        let local = cfg.load();
        let state = self.live_state.lock();
        let info = WifiDeviceInfo {
            nodes: state.nodes.clone(),
            remote_addr: self.remote_addr.clone(),
            name: self.name.clone(),
            mac: self.mac.clone(),
            rssi: 0,
            esp_model: ESP32Model::Unknown,
            intensity: local.intensity,
            offset: local.offset,
        };
        DeviceInfo::Wifi(info)
    }

    /// currently only updates the nodes.
    fn update_info(&self, new: DeviceInfo) {
        match new {
            DeviceInfo::Wifi(inf) => {
                let (_, cfg) = state::get_device(&DeviceId(self.mac.clone()));
                let mut local = PerDevice::clone(&cfg.load());
                local.intensity = inf.intensity;
                local.offset = inf.offset;
                cfg.swap(Arc::new(local));

                let mut state = self.live_state.lock();
                if let Some( ref mut conf) = &mut state.config {
                    conf.node_map = inf.nodes.clone();
                    
                }
                state.output.write().resize(inf.nodes.len(), 0.0);
                state.nodes = inf.nodes;
                state.push_map = true; // signal to persist to device
                log_err!(self.manager.try_send(DeviceMessage::InfoDirty(self.get_id()))); // tell map our info has changed
            } 
            _ => log::warn!("Updated with wrong info type on wifi device: {:?}=>{:?}", self.name, self.mac),
        }
    }

    fn disconnect(&mut self) {
        self.cancel.cancel();
    }

    fn get_feedback_buffer(&self) -> Arc<RwLock<Vec<f32>>> {
        let state = self.live_state.lock();
        Arc::clone(&state.output)
    }

    /// Does nothing since device continously updated.
    fn buffer_updated(&self) {
        
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
    NewConfig(Box<WifiConfig>),
    /// Should set wifi config to None, since we just changed it's value.
    ResetConfig,
    NewIdentifier(ESP32Model),
    NewHeartBeat(Instant),
    PingConfirmation,
}
