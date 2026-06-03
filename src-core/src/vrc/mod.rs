pub mod cache_node;
pub mod config;
pub mod discovery;
pub mod osc_query;
pub mod parsing;

// crate dependencies
use crate::api::ApiManager;
use crate::mapping::input_node::{Slot, SlotKey};
use crate::mapping::{
    input_node::InputNode,
};
use crate::mapping::{InputEventMessage, MapHandle, NodeField, NodeFieldKind, NodeKey, SlotField, SlotFieldKind};
use crate::osc::server::OscServer;
use crate::state::{self, VrcSettings};
use crate::vrc::config::InputType;
use arc_swap::{ArcSwap, Cache, Guard};
use glam::Vec3;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use tokio::sync::oneshot;
use crate::vrc::parsing::OscInfo;
use crate::{log_err};

// module dependencies
use config::GameMap;
use dashmap::DashMap;
use discovery::start_filling_available_parameters;
use hazarc::{ArcBorrow, AtomicArc};
use osc_query::OscQueryServer;
use parsing::remove_version;

use rosc::{OscMessage, OscType};
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use std::{net::Ipv4Addr, sync::Arc};
use tokio::sync::{
    mpsc::{channel, Receiver, Sender},
    Mutex,
};

/// struct exposed to the UI.
///
/// Seperates serializable with under the hood specifications.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct VrcInfo {
    pub is_connected: bool,
    pub in_port: Option<u16>,
    pub out_port: Option<u16>,
    pub avatar: Option<Avatar>,
    pub velocity_ratio: f32,
    pub velocity_mult: f32,
    pub watched: Vec<(String, AddrInfo)>,
    pub available: Vec<OscInfo>,
}

impl Default for VrcInfo {
    fn default() -> Self {
        Self {
            is_connected: false,
            in_port: None,
            out_port: None,
            avatar: None,
            velocity_ratio: 0.5,
            velocity_mult: 0.5,
            watched: vec![],
            available: Vec::new(),
        }
    }
}

static CHECKING: std::sync::Mutex<f32> = std::sync::Mutex::new(0.0);

// "/avatar/parameters/haptic/prefabs/<author>/<name>/<version>"
// I think having trailing "/" references the contents of the path, not all the children paths.
pub const PREFAB_PREFIX: &str = "/avatar/parameters/haptic/prefabs/";
pub const INTENSITY_PATH: &str = "/avatar/parameters/haptic/global/intensity";
pub const ENABLE_PATH: &str = "/avatar/parameters/haptic/global/enable";
pub const AVATAR_ID_PATH: &str = "/avatar/change";
pub const VRC_TAG: &str = "VRC";

/// Implements cheap clone, is threadsafe.
pub struct VrcHandle {
    tx: Sender<MsgToMainVrc>,
    osc_buffer: Arc<std::sync::Mutex<Vec<OscMessage>>>,
    flush_scheduled: Arc<std::sync::atomic::AtomicBool>,
    info: Arc<ArcSwap<VrcInfo>>,
}

impl VrcHandle {
    pub fn send_osc_msg_rcv(&self, msg: OscMessage) {
        log_err!(self.tx.try_send(MsgToMainVrc::Osc(msg)));
    }

    fn flush_osc(&self) {
        let batch = {
            let mut guard = self.osc_buffer.lock().unwrap();
            if guard.is_empty() { return; }
            std::mem::take(&mut *guard)
        };
        let _ = self.tx.try_send(MsgToMainVrc::OscBatch(batch));
    }

    pub fn blocking_send(&self, msg: MsgToMainVrc) {
        log_err!(self.tx.try_send(msg));
    }

    pub async fn send(&self, msg: MsgToMainVrc) {
        log_err!(self.tx.send(msg).await);
    }

    pub async fn get_info(&self) -> Arc<VrcInfo> {
        let (tx, rx) = oneshot::channel();
        self.blocking_send(MsgToMainVrc::Info(tx));
        log_err!(rx.await);
        self.info.load_full()
    }
}

impl Clone for VrcHandle {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            osc_buffer: Arc::clone(&self.osc_buffer),
            flush_scheduled: Arc::clone(&self.flush_scheduled),
            info: Arc::clone(&self.info),
        }
    }
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum AddrInfo
{
    Slot(SlotKey, SlotFieldKind),
    Node(NodeKey, NodeFieldKind)
}

pub struct VrcGame {
    recv_port: u16,
    ui_info: Arc<ArcSwap<VrcInfo>>,
    handle: VrcHandle,
    /// Holds data from http server about the given avatar
    pub avatar: Option<Avatar>,
    /// Parameters VRC advertises as available, is empty if not resolved yet
    ///
    /// NOTE: The values actual values contained in this struct are out of date by up to 2 seconds.
    pub available_parameters: Arc<DashMap<OscPath, OscInfo>>,
    pub watched: FxHashMap<String, SmallVec<[AddrInfo; 2]>>,
    rx: Receiver<MsgToMainVrc>,
    map: MapHandle,
    /// The OSC server we receive updates from
    osc_server: OscServer,
    /// Spawns our own OSCQuery advertising
    query_server: Option<OscQueryServer>,
    dirty: bool,
}

/// I hate naming things
//const _: [u8; std::mem::size_of::<MsgToMainVrc>()] = [];
#[derive(Debug)]
pub enum MsgToMainVrc {
    Info(oneshot::Sender<()>),
    Osc(OscMessage),
    /// message is received from the OSC server
    OscBatch(Vec<OscMessage>),
    /// A new avatar configuration was detected
    NewAvatar(Avatar),
    VrcDisconnected,
}

impl VrcGame {
    pub async fn new(map_handle: MapHandle, api: &'static Mutex<ApiManager>) -> VrcGame {
        log::trace!("Starting VRC");
        let (tx, rx) = channel(50);
        let info = Arc::new(ArcSwap::new(Arc::new(VrcInfo::default().into())));
        let handle = VrcHandle {
            tx: tx.clone(),
            info: Arc::clone(&info),
            flush_scheduled: Arc::new(AtomicBool::new(false)),
            osc_buffer: Arc::new(std::sync::Mutex::new(vec![])),
        };
        let param_avail = Arc::new(DashMap::new());

        //create the low-latency server.
        let handle_rcv = handle.clone();
        let on_receive = move |msg: OscMessage| {
            handle_rcv.send_osc_msg_rcv(msg);
        };
        let receiving_port = 9001;
        let mut vrc_server = OscServer::new(receiving_port, Ipv4Addr::UNSPECIFIED, on_receive);
        let port_used = vrc_server.start().await;

        // Instantiate
        let vrc = VrcGame {
            recv_port: port_used,
            ui_info: info,
            handle: handle.clone(),
            osc_server: vrc_server,
            query_server: None,
            avatar: None,
            rx: rx,
            map: map_handle,
            available_parameters: Arc::clone(&param_avail),
            watched: FxHashMap::default(),
            dirty: false,
        };

        // Start the thread that handles finding available vrc parameters
        // (High latency server)
        start_filling_available_parameters(vrc.get_handle(), api, param_avail).await;

        // if the server wasn't able to capture the port start advertising the port it was bound to.
        if port_used != receiving_port {
            let mut osc_server = OscQueryServer::new(receiving_port);
            osc_server.start().await;
            log::warn!("Not using VRC dedicated ports, expect slower operations.");
        }

        return vrc;
    }

    pub fn get_handle(&self) -> VrcHandle {
        self.handle.clone()
    }

    /// Main event loop that handles VRC communications.
    pub async fn run(&mut self) {
        log::trace!("Running VRC");
        let mut settings = Cache::new(&state::get_config().vrc_settings);
        loop {
            let Some(msg) = self.rx.recv().await else {
                log::warn!("Shutting down vrc game");
                return;
            };

            match msg {
                MsgToMainVrc::Info(tx) => {
                    self.update_info(); // updates if dirty
                    log_err!(tx.send(())); // marks we are done updating
                }
                MsgToMainVrc::OscBatch(batch) => {
                    let cfg = settings.load();
                    self.process_osc_batch(&batch, cfg);

                    self.dirty = true;
                }
                MsgToMainVrc::Osc(msg) => {
                    let cfg = settings.load();
                    self.process_msg(&msg, cfg);
                    self.dirty = true;
                }
                MsgToMainVrc::NewAvatar(avi) => {
                    //clear current input nodes
                    log_err!(self.map
                        .send_event(InputEventMessage::Flush));

                    let cfg = settings.load();
                    let nodes = to_inputs(&avi, cfg);
                    self.avatar = Some(avi);

                    if !nodes.is_empty() {
                        for (pairs, node) in nodes {
                            let (tx, rx) = oneshot::channel();
                            log_err!(self.map
                                .send_event(InputEventMessage::Register { node: node, reply: tx }));
                            let Ok(resp) = rx.await else {
                                log::error!("Error registering haptic node. Shutting down VRC Module");
                                return;
                            };

                            for (addr, idx) in pairs {
                                let key = SlotKey::new(resp, idx);
                                self.watched
                                    .entry(addr.clone())
                                    .or_default()
                                    .push(AddrInfo::Slot(key, SlotFieldKind::Value));
                            }
                        }
                    }
                    self.dirty = true;
                }
                MsgToMainVrc::VrcDisconnected => {
                    log::warn!("Vrc Disconnected");
                    self.avatar = None;
                    self.available_parameters.clear();
                    self.dirty = true;
                }
            }
        }
    }

    /// clones all our info into a new instance that will be swapped out.
    fn update_info(&mut self) {
        if !self.dirty {
            return;
        }

        let current = &self.ui_info;
        let changed = VrcInfo {
            is_connected: !self.available_parameters.is_empty(),
            in_port: Some(self.recv_port),
            out_port: Some(0),
            available: self.available_parameters.iter().map(|entry| entry.value().clone()).collect(),
            avatar: self.avatar.clone(),
            velocity_mult: 0.0, // these will be filled out by touching the state in the handle function
            velocity_ratio: 0.0,
            watched: self.watched.iter()
                .flat_map(|(s, infos)| infos.iter().map(move |i| (s.clone(), i.clone())))
                .collect(),
        };

        current.swap(Arc::new(changed));
        self.dirty = false;
    }

    fn process_osc_batch(&self, batch: &[OscMessage], cfg: &VrcSettings) {
        let Some(avatar) = self.avatar.as_ref() else {
            return;
        };

        if avatar.configs.is_empty() {
            return;
        };

        for msg in batch {
            let addr = remove_version(&msg.addr);
            let Some(arg) = msg.args.first() else { return };

            let Some(infos) = self.watched.get(&addr) else {
                continue; // not one we watch for.
            };

            for info in infos {
                match info {
                    AddrInfo::Slot(s, kind) => {
                        let msg = match kind {
                            SlotFieldKind::Weight => InputEventMessage::UpdateSlotField { key: s.clone(), field: SlotField::Weight(try_f32(arg.clone())) },
                            SlotFieldKind::Value => InputEventMessage::UpdateSlotField { key: s.clone(), field: SlotField::Value(try_f32(arg.clone())) },
                            SlotFieldKind::Muted => InputEventMessage::UpdateSlotField { key: s.clone(), field: SlotField::Muted(try_bool(arg.clone())) },
                        };
                        log_err!(self.map.send_event(msg));
                    },
                    AddrInfo::Node(n, kind) => {
                        let msg = match kind {
                            NodeFieldKind::Location => InputEventMessage::UpdateNodeField { key: n.clone(), field: NodeField::Location(try_vec(arg.clone())) },
                            NodeFieldKind::Radius => InputEventMessage::UpdateNodeField { key: n.clone(), field: NodeField::Radius(try_f32(arg.clone())) },
                            NodeFieldKind::Muted => InputEventMessage::UpdateNodeField { key: n.clone(), field: NodeField::Muted(try_bool(arg.clone())) },
                        };
                        log_err!(self.map.send_event(msg));
                    }
                }
            }
        }
    }

    fn process_msg(&self, msg: &OscMessage, cfg: &VrcSettings) {
        let addr = remove_version(&msg.addr);
        let Some(arg) = msg.args.first() else { return };
        let Some(infos) = self.watched.get(&addr) else {
            return; // not one we watch for.
        };

        for info in infos {
            match info {
                AddrInfo::Slot(s, kind) => {
                    let msg = match kind {
                        SlotFieldKind::Weight => InputEventMessage::UpdateSlotField { key: s.clone(), field: SlotField::Weight(try_f32(arg.clone())) },
                        SlotFieldKind::Value => InputEventMessage::UpdateSlotField { key: s.clone(), field: SlotField::Value(try_f32(arg.clone())) },
                        SlotFieldKind::Muted => InputEventMessage::UpdateSlotField { key: s.clone(), field: SlotField::Muted(try_bool(arg.clone())) },
                    };
                    log_err!(self.map.send_event(msg));
                },
                AddrInfo::Node(n, kind) => {
                    let msg = match kind {
                        NodeFieldKind::Location => InputEventMessage::UpdateNodeField { key: n.clone(), field: NodeField::Location(try_vec(arg.clone())) },
                        NodeFieldKind::Radius => InputEventMessage::UpdateNodeField { key: n.clone(), field: NodeField::Radius(try_f32(arg.clone())) },
                        NodeFieldKind::Muted => InputEventMessage::UpdateNodeField { key: n.clone(), field: NodeField::Muted(try_bool(arg.clone())) },
                    };
                    log_err!(self.map.send_event(msg));
                }
            }
        }
        
    }
}

const F32DEFAULT: f32 = 0.0;
const VECDEFAULT: Vec3 = Vec3::ZERO;

/// I know this is somewhere else in the codebase but i don't care.
/// Defaults to zero array and inserts first value into x if singular value.
fn try_vec(val: OscType) -> Vec3 {
    match val {
        OscType::Float(f) => Vec3::new(f, 0.0, 0.0),
        OscType::Array(a) => {
            let x = try_f32(a.content.get(0).unwrap_or(&OscType::Float(F32DEFAULT)).clone());
            let y = try_f32(a.content.get(1).unwrap_or(&OscType::Float(F32DEFAULT)).clone());
            let z = try_f32(a.content.get(2).unwrap_or(&OscType::Float(F32DEFAULT)).clone());
            Vec3::new(x, y, z)
        },
        OscType::Double(d) => Vec3::new(d as f32, 0.0, 0.0),
        // TODO: Look into better casting these values.
        _ => VECDEFAULT,

    }
}

/// I know this is somewhere else in the codebase but i don't care.
/// Defaults to 0.0, and first element of an array.
fn try_f32(val: OscType) -> f32 {
    match val {
        OscType::Float(f) => f,
        OscType::Array(a) => {
            let first = a.content.get(0).unwrap_or(&OscType::Float(F32DEFAULT));
            try_f32(first.clone())
        },
        OscType::Bool(b) => f32::from(u8::from(b)),
        OscType::Double(d) => d as f32,
        OscType::Inf => 1.0,
        // TODO: Look into better casting these values.
        _ => F32DEFAULT,

    }
}

/// I know this is somewhere else in the codebase but i don't care.
/// Defaults to false
fn try_bool(val: OscType) -> bool {
    match val {
        OscType::Float(f) => f > 0.5,
        OscType::Array(a) => {
            let first = a.content.get(0).unwrap_or(&OscType::Float(F32DEFAULT));
            try_bool(first.clone())
        },
        OscType::Bool(b) => b,
        OscType::Double(d) => d > 0.5,
        OscType::Inf => true,
        // TODO: Look into better casting these values.
        _ => false,

    }
}
 
/// Converts a vrc avatar descriptor into a list of input nodes for our input map.
fn to_inputs(avi: &Avatar, settings: &Arc<VrcSettings>) -> Vec<(Vec<(String, u8)>, Box<InputNode>)> {
    let mut nodes = vec![];

    for conf in &avi.configs {
        for node in &conf.nodes {
            let location = node.location;
            let layer = &node.interpolation_layer;
            let groups  = node.interaction_tags;
            let radius = node.radius;
            
            let vel = settings.velocity_ratio;
            let slots: Vec<(String, Slot)> = node.inputs.iter().map(|i| {
                let weight = match i.source {
                    InputType::Weight => vel,
                    InputType::Velocity => 1.0 - vel,
                };

                return (
                    i.vrc_prefix.to_owned() + &i.address.to_owned(), 
                    Slot {
                        muted: false,
                        source: i.source.clone(),
                        layer: i.layer.clone(),
                        weight: weight,
                        value: 0.0,
                        history: SmallVec::new(),
                    }
                );
            }).collect();

            let mut node = Box::new(InputNode::new(location, groups, layer.clone(), SmallVec::new(), radius));
            let mut addrs = vec![];
            for (idx, (addr, slot)) in slots.iter().enumerate() {
                node.slots.push(slot.clone());
                addrs.push((addr.clone(), idx as u8));
            }

            nodes.push((addrs, node));
        }
    }

    nodes
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
/// Abstraction over raw VRC parameters.
///
/// Represents all relevant *Descriptive* data. Does not contain any relevant high-speed or low latency datat.
pub struct Avatar {
    /// The avatar referred to by the VRC api
    id: String,
    /// the names of the prefabs from the avatar parameter
    prefab_names: Vec<String>,
    /// All information mapping OSC Parameters to their needed formats
    configs: Vec<GameMap>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
/// Simple wrapper for the String class.
/// Represnts a full OscPath without any elements stripped,
/// other than the VRC Fury naming.
pub struct OscPath(pub String);
