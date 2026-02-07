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
use crate::state::{self, Config, cache};
use crate::vrc::parsing::OscInfo;

// module dependencies
use cache_node::CacheNode;
use config::GameMap;
use dashmap::DashMap;
use discovery::start_filling_available_parameters;
use osc_query::OscQueryServer;
use parsing::remove_version;
use hazarc::{AtomicArc, ArcBorrow};

use rosc::OscMessage;
use std::{
    net::Ipv4Addr,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc::{channel, Receiver, Sender};

/// struct exposed to the UI.
///
/// Seperates serializable with under the hood specifications.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct VrcInfo {
    pub is_connected: bool,
    pub in_port: Option<u16>,
    pub out_port: Option<u16>,
    pub avatar: Option<Avatar>,
}

impl Default for VrcInfo {
    fn default() -> Self {
        Self {
            is_connected: false,
            in_port: None,
            out_port: None,
            avatar: None,
        }
    }
}

// "/avatar/parameters/haptic/prefabs/<author>/<name>/<version>"
// I think having trailing "/" references the contents of the path, not all the children paths.
pub const PREFAB_PREFIX: &str = "/avatar/parameters/haptic/prefabs/";
pub const INTENSITY_PATH: &str = "/avatar/parameters/haptic/global/intensity";
pub const AVATAR_ID_PATH: &str = "/avatar/change";
pub const VRC_TAG: &str = "VRC";

/// Implements cheap clone, is threadsafe.
pub struct VrcHandle {
    tx: Sender<MsgToMainVrc>,
    info: Arc<AtomicArc<VrcInfo>>,
}

impl VrcHandle {
    pub async fn send_osc_msg_rcv(&self, msg: OscMessage) {
        self.tx.send(MsgToMainVrc::OscMessageRecieved(msg)).await;
    }

    pub fn send(&self, msg: MsgToMainVrc) {
        self.tx.blocking_send(msg);
    }

    pub fn get_info(&self) -> ArcBorrow<VrcInfo> {
        self.info.load()
    }
}

impl Clone for VrcHandle {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            info: Arc::clone(&self.info),
        }
    }
}

pub struct VrcGame {
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
    osc_server: Option<OscServer>,
    /// Spawns our own OSCQuery advertising
    query_server: Option<OscQueryServer>,
}

/// I hate naming things
pub enum MsgToMainVrc {
    /// Pushes our cache to the map state.
    RefreshMap,
    /// message is recieved from the OSC server
    OscMessageRecieved(OscMessage),
    /// A new avatar configuration was detected
    NewAvatar(Avatar),
    VrcDisconnected,
}

impl VrcGame {
    pub async fn new(map_handle: MapHandle, api: &'static Mutex<ApiManager>) -> VrcGame {
        let (tx, rx) = channel(10);
        let info = Arc::new(AtomicArc::new(VrcInfo::default().into()));
        let handle = VrcHandle { tx: tx.clone(), info: Arc::clone(&info) };
        let param_cache = Arc::new(DashMap::new());
        let param_avail = Arc::new(DashMap::new());

        // Instantiate
        let vrc = VrcGame {
            ui_info: info,
            handle: handle.clone(),
            osc_server: None,
            query_server: None,
            avatar: None,
            rx: rx,
            map: map_handle,
            available_parameters: Arc::clone(&param_avail),
            parameter_cache: Arc::clone(&param_cache),
        };

        // Start the thread that handles finding available vrc parameters
        // (High latency server)
        start_filling_available_parameters(vrc.get_handle(), api, param_avail);

        //create the low-latency server.
        let handle_rcv = handle.clone();
        let on_receive = move |msg: OscMessage| {
            handle_rcv.send_osc_msg_rcv(msg);
        };

        let recieving_port = 9001;
        let mut vrc_server = OscServer::new(recieving_port, Ipv4Addr::LOCALHOST, on_receive);
        let port_used = vrc_server.start();

        // if the server wasn't able to capture the port start advertising the port it was bound to.
        if port_used != recieving_port {
            let mut osc_server = OscQueryServer::new(recieving_port);
            osc_server.start();
            log::warn!("Not using VRC dedicated ports, expect slower operations.");
        }

        return vrc;
    }

    pub fn get_handle(&self) -> VrcHandle {
        self.handle.clone()
    }

    /// Main event loop that handles VRC communications.
    pub async fn run(&mut self) {
        let mut cfg_cache = cache();
        loop {
            let Some(msg) = self.rx.recv().await else {
                log::warn!("Shutting down vrc game");
                return;
            };

            match msg {
                // called at high velocity. 
                MsgToMainVrc::RefreshMap => {
                    let cfg = cfg_cache.load();
                    self.refresh_map(cfg, &self.map);
                }
                MsgToMainVrc::OscMessageRecieved(msg) => {
                    // remove VRC Fury tagging if needed
                    let addr = remove_version(&msg.addr);

                    // if there is a value push it to our cache.
                    if let Some(arg) = msg.args.first() {
                        // if we have a cache going otherwise build it.
                        if let Some(mut cache) =
                            self.parameter_cache.get_mut(&OscPath(addr.clone()))
                        {
                            let _ = cache.update(arg.to_owned());
                        } else {
                            self.parameter_cache.insert(
                                OscPath(addr),
                                CacheNode::new(
                                    arg.to_owned(),
                                    state::clone_field(|c| &c.vrc_settings.sample_cache),
                                    state::clone_field(|c| &c.vrc_settings.smoothing_time),
                                    0.2,
                                    1.0,
                                    1.0,
                                ),
                            );
                        }

                        // push changes to
                    } else {
                        log::warn!("empty message recieved: {:?}", msg);
                    }
                }
                MsgToMainVrc::NewAvatar(avi) => {
                    log::debug!("New avatar: {avi:?}");
                    //clear current input nodes
                    self.map
                        .send_event(InputEventMessage::RemoveWithTags(vec![VRC_TAG.into()]));

                    let nodes = to_inputs(&avi);
                    self.avatar = Some(avi);

                    if !nodes.is_empty() {
                        for node in nodes {
                            self.map
                                .send_event(InputEventMessage::InsertNode(node))
                                .await;
                        }
                    }
                },
                MsgToMainVrc::VrcDisconnected => {
                    log::warn!("Vrc Disconnected");
                    self.avatar = None;
                    self.available_parameters.clear();
                }
            }
        }
    }

    /// Propogates our cached values to changes on the input map.
    fn refresh_map(&self, cfg: &Config, map: &MapHandle) {
        let Some(avatar) = self.avatar.as_ref() else {
            return;
        };

        if avatar.configs.is_empty() {
            return;
        };

        // update menu items (VERY CURSED)
        let mut collect = vec![];
        let mut cfg_int = 0.0;
        if let Some(int_node) = self.parameter_cache.get(&OscPath(INTENSITY_PATH.into())) {
            cfg_int = int_node.raw_last();
            if (cfg.mapping_menu.intensity - cfg_int).abs() > 0.01 {
                collect.push(|d: &mut Config| {d.mapping_menu.intensity = cfg_int});
            }
        }

        if !collect.is_empty() {
            let mut new = Config::clone(cfg);
            for f in collect {
                f(&mut new);
            }
            state::swap(new);
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
                        cache_node.set_position_weight(1.0 - cfg.vrc_settings.velocity_ratio);
                        cache_node.set_velocity_mult(cfg.vrc_settings.velocity_mult);
                        cache_node.set_contact_scale(1.0);
                        n.set_intensity(cache_node.latest());
                    });
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

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
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

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
/// Simple wrapper for the String class.
/// Represnts a full OscPath without any elements stripped,
/// other than the VRC Fury naming.
pub struct OscPath(pub String);
