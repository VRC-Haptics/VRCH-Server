pub mod cache_node;
pub mod config;
pub mod discovery;
pub mod osc_query;
pub mod parsing;

// crate dependencies
use crate::api::ApiManager;
use crate::mapping::{
    input_node::{InputNode, InputType},
    NodeId,
};
use crate::mapping::{InputEventMessage, MapHandle};
use crate::osc::server::OscServer;
use crate::state::{self, StandardMenu, VrcSettings};
use arc_swap::Cache;
use tokio::task::JoinHandle;
use crate::vrc::parsing::OscInfo;
use crate::{log_err};

// module dependencies
use cache_node::CacheNode;
use config::GameMap;
use dashmap::DashMap;
use discovery::start_filling_available_parameters;
use hazarc::{ArcBorrow, AtomicArc};
use osc_query::OscQueryServer;
use parsing::remove_version;
use rayon::prelude::*;

use rosc::OscMessage;
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
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, specta::Type)]
pub struct VrcInfo {
    pub is_connected: bool,
    pub in_port: Option<u16>,
    pub out_port: Option<u16>,
    pub avatar: Option<Avatar>,
    pub velocity_ratio: f32,
    pub velocity_mult: f32,
    pub cached: Vec<(OscPath, CacheNode)>,
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
            cached: Vec::new(),
            available: Vec::new(),
        }
    }
}

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
    info: Arc<AtomicArc<VrcInfo>>,
}

impl VrcHandle {
    pub fn send_osc_msg_rcv(&self, msg: OscMessage) {
        {
            let mut guard = self.osc_buffer.lock().unwrap();
            guard.push(msg);
        }

        // Schedule a flush if one isn't already pending
        if !self.flush_scheduled.swap(true, std::sync::atomic::Ordering::AcqRel) {
            let handle = self.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(50)).await;
                handle.flush_osc();
                handle.flush_scheduled.store(false, std::sync::atomic::Ordering::Release);
            });
        }
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

    pub fn get_info(&self) -> VrcInfo {
        let settings = state::get_config().vrc_settings.load();
        let mut new = VrcInfo::clone(&self.info.load());
        new.velocity_ratio = settings.velocity_ratio;
        new.velocity_mult = settings.velocity_mult;
        new
    }

    pub fn get_info_ref(&self) -> ArcBorrow<VrcInfo> {
        self.info.load()
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

pub struct VrcGame {
    recv_port: u16,
    ui_info: Arc<AtomicArc<VrcInfo>>,
    handle: VrcHandle,
    /// Holds data from http server about the given avatar
    pub avatar: Option<Avatar>,
    /// Parameters VRC advertises as available, is empty if not resolved yet
    ///
    /// NOTE: The values actual values contained in this struct are out of date by up to 2 seconds.
    pub available_parameters: Arc<DashMap<OscPath, OscInfo>>,
    /// Buffer that is filled with values collected from the OSC stream.
    /// If the buffer doesn't contain value it hasn't been seen since last flush.
    pub parameter_cache: Arc<DashMap<OscPath, CacheNode>>,
    rx: Receiver<MsgToMainVrc>,
    map: MapHandle,
    /// The OSC server we recieve updates from
    osc_server: OscServer,
    /// Spawns our own OSCQuery advertising
    query_server: Option<OscQueryServer>,
}

/// I hate naming things
#[derive(Debug)]
pub enum MsgToMainVrc {
    /// Pushes our cache to the map state.
    RefreshMap,
    /// message is recieved from the OSC server
    OscBatch(Vec<OscMessage>),
    /// A new avatar configuration was detected
    NewAvatar(Avatar),
    VrcDisconnected,
}

impl VrcGame {
    pub async fn new(map_handle: MapHandle, api: &'static Mutex<ApiManager>) -> VrcGame {
        log::trace!("Starting VRC");
        let (tx, rx) = channel(50);
        let info = Arc::new(AtomicArc::new(VrcInfo::default().into()));
        let handle = VrcHandle {
            tx: tx.clone(),
            info: Arc::clone(&info),
            flush_scheduled: Arc::new(AtomicBool::new(false)),
            osc_buffer: Arc::new(std::sync::Mutex::new(vec![])),
        };
        let param_cache = Arc::new(DashMap::new());
        let param_avail = Arc::new(DashMap::new());

        //create the low-latency server.
        let handle_rcv = handle.clone();
        let on_receive = move |msg: OscMessage| {
            handle_rcv.send_osc_msg_rcv(msg);
        };
        let recieving_port = 9001;
        let mut vrc_server = OscServer::new(recieving_port, Ipv4Addr::LOCALHOST, on_receive);
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
            parameter_cache: Arc::clone(&param_cache),
        };

        // Start the thread that handles finding available vrc parameters
        // (High latency server)
        start_filling_available_parameters(vrc.get_handle(), api, param_avail).await;

        // if the server wasn't able to capture the port start advertising the port it was bound to.
        if port_used != recieving_port {
            let mut osc_server = OscQueryServer::new(recieving_port);
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

                // called at high velocity.
                MsgToMainVrc::RefreshMap => {

                    self.refresh_map(&self.map, &settings.load());
                }
                MsgToMainVrc::OscBatch(batch) => {
                    let cfg = settings.load();
                    self.process_osc_batch(&batch, cfg);

                    self.refresh_map(&self.map, cfg);
                    self.update_info();
                }
                MsgToMainVrc::NewAvatar(avi) => {
                    //clear current input nodes
                    log_err!(self.map
                        .send_event(InputEventMessage::RemoveWithTags(vec![VRC_TAG.into()]))
                        .await);

                    let nodes = to_inputs(&avi);
                    self.avatar = Some(avi);

                    if !nodes.is_empty() {
                        for node in nodes {
                            log_err!(self.map
                                .send_event(InputEventMessage::InsertNode(node))
                                .await);
                        }
                    }
                    self.update_info();
                }
                MsgToMainVrc::VrcDisconnected => {
                    log::warn!("Vrc Disconnected");
                    self.avatar = None;
                    self.available_parameters.clear();
                    self.update_info();
                }
            }
        }
    }

    /// clones all our info into a new instance that will be swapped out.
    fn update_info(&mut self) {
        let current = &self.ui_info;
        let changed = VrcInfo {
            is_connected: !self.available_parameters.is_empty(),
            in_port: Some(self.recv_port),
            out_port: Some(0),
            available: self.available_parameters.iter().map(|entry| entry.value().clone()).collect(),
            avatar: self.avatar.clone(),
            velocity_mult: 0.0, // these will be filled out by touching the state in the handle function
            velocity_ratio: 0.0,
            cached: self.parameter_cache.iter()
    .map(|entry| (entry.key().clone(), entry.value().clone()))
    .collect(),
        };

        current.swap(Arc::new(changed));
    }

    fn process_osc_batch(&self, batch: &[OscMessage], cfg: &VrcSettings) {
        // DashMap supports concurrent writes — process in parallel
        batch.par_iter().for_each(|msg| {
            let addr = remove_version(&msg.addr);
            let Some(arg) = msg.args.first() else { return };

            let key = OscPath(addr);
            if let Some(mut cache) = self.parameter_cache.get_mut(&key) {
                let _ = cache.update(arg.to_owned().into());
            } else {
                self.parameter_cache.insert(
                    key,
                    CacheNode::new(
                        arg.to_owned(),
                        cfg.sample_cache.clone(),
                        cfg.smoothing_time.clone(),
                        0.2, 1.0, 1.0,
                    ),
                );
            }
        });
    }

    /// Propogates our cached values to changes on the input map.
    fn refresh_map(&self, map: &MapHandle, settings: &VrcSettings) {
        let Some(avatar) = self.avatar.as_ref() else {
            return;
        };

        if avatar.configs.is_empty() {
            return;
        };

        // update menu items and only push when needed
        let int_node = self.parameter_cache.get(&OscPath(INTENSITY_PATH.into()));
        let en_node = self.parameter_cache.get(&OscPath(ENABLE_PATH.into()));
        match (int_node, en_node) {
            (Some(int), Some(en)) =>  {
                let intensity = int.raw_last().clamp(0.0, 1.0);
                let enable = en.raw_last();
                let enable = enable > 0.5; 

                let menu = state::get_config().mapping_menu.load();
                if menu.intensity - intensity < 0.05 || enable != menu.enable {
                    let mut new = StandardMenu::clone(&menu);
                    new.intensity = intensity;
                    new.enable = enable;
                    state::get_config().mapping_menu.store(new.into());
                }
            },
            (int, en) => {
                if int.is_none() && en.is_none() {
                    return;
                }

                if let Some(intensity) = int {
                    let intensity = intensity.raw_last().clamp(0.0, 1.0);
                    let menu = state::get_config().mapping_menu.load();
                    if menu.intensity - intensity < 0.05 {
                        let mut new = StandardMenu::clone(&menu);
                        new.intensity = intensity;
                        state::get_config().mapping_menu.store(new.into());
                    }
                }

                if let Some(enable) = en {
                    let en = enable.raw_last() > 0.5;
                    let menu = state::get_config().mapping_menu.load();
                    if menu.enable != en {
                        let mut new = StandardMenu::clone(&menu);
                        new.enable = en;
                        state::get_config().mapping_menu.store(new.into());
                    }
                }
            }
        }

        // update input nodes
        for conf in &avatar.configs {
            for node in &conf.nodes {
                if let Some(mut cache_node) =
                    self.parameter_cache.get_mut(&OscPath(node.address.clone()))
                {
                    // update node if already created
                    let id = NodeId(node.address.clone());
                    map.with_node_mut(&id, |n| {
                        if node.is_external_address {
                            n.set_intensity(cache_node.raw_last());
                            return;
                        }

                        // update our cache node, then read output to our nodes.
                        cache_node.set_position_weight(1.0 - settings.velocity_ratio);
                        cache_node.set_velocity_mult(settings.velocity_mult);
                        cache_node.set_contact_scale(1.0);
                        n.set_intensity(cache_node.latest());
                    });
                } else {
                    log::warn!("node in config but not found in map: {:?}", node);
                }
            } // for loop

            // tell map we are done messing with it
            // blocks till it sends.
            map.mark_dirty_blocking();
        }
    }

    /// Purges the parameter cache.
    pub fn purge_cache(&mut self) {
        self.parameter_cache.clear();
        log::info!("Purged Parameter cache.");
    }
}

/// Converts a vrc avatar descriptor into a list of input nodes for our input map.
fn to_inputs(avi: &Avatar) -> Vec<InputNode> {
    let mut nodes = vec![];

    for conf in &avi.configs {
        for node in &conf.nodes {
            let mut haptic_node = node.node_data.clone();
            if node.is_external_address {
                haptic_node.groups.push(crate::mapping::NodeGroup::All);
            }

            let mut input_type = InputType::INTERP;
            if node.is_external_address {
                input_type = InputType::ADDITIVE
            }

            nodes.push(InputNode::new(
                haptic_node,
                vec![
                    format!(
                        "{}_{}_{}",
                        conf.meta.map_author, conf.meta.map_name, conf.meta.map_version
                    ),
                    node.target_bone.to_string(),
                    "vrc_config_node".to_string(),
                    VRC_TAG.into(),
                ],
                NodeId(node.address.clone()),
                node.radius,
                input_type,
            ));
        }
    }

    nodes
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, specta::Type)]
/// Abstraction over raw VRC parameters.
///
/// Represents all relevant *Descriptive* data. Does not contain any relevant high-speed or low latency datat.
pub struct Avatar {
    /// The avatar reffered to by the VRC api
    id: String,
    /// the names of the prefabs from the avatar parameter
    prefab_names: Vec<String>,
    /// All information mapping OSC Parameters to their needed formats
    configs: Vec<GameMap>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash, specta::Type)]
/// Simple wrapper for the String class.
/// Represnts a full OscPath without any elements stripped,
/// other than the VRC Fury naming.
pub struct OscPath(pub String);
