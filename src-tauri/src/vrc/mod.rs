pub mod discovery;
pub mod parsing;
mod config;

// crate dependencies
use crate::osc::server::OscServer;
use crate::GlobalMap;
use crate::vrc::parsing::OscInfo;
use crate::mapping::{input_node::InputNode, Id};
// module dependencies
use parsing::remove_version;
use config::GameMap;
use discovery::{OscQueryServer, start_filling_available_parameters};
use rosc::{OscType, OscMessage};
use dashmap::DashMap;
// std imports
use std::{
    net::Ipv4Addr,
    sync::{Arc, Mutex, RwLock},
};

// "/avatar/parameters/haptic/prefabs/<author>/<name>/<version>"
// I think having trailing "/" references the contents of the path, not all the children paths.
pub const AVATAR_PREFIX: &str = "/avatar/parameters";
pub const PREFAB_PREFIX: &str = "/avatar/parameters/haptic/prefabs/";
pub const AVATAR_ID_PATH: &str = "/avatar/change";

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct VrcInfo {
    /// whether we are currently connected to a VRChat client
    pub vrc_connected: bool,
    /// port we recieve the low-latency OSC data on
    pub in_port: Option<u16>,
    /// port we are sending data over
    pub out_port: Option<u16>,
    /// Holds data from http server about the given avatar
    pub avatar: Arc<RwLock<Option<Avatar>>>,
    /// Parameters VRC advertises as available, is empty if not resolved yet
    /// 
    /// NOTE: The values actual values contained in this struct are out of date by ~2S.
    pub available_parameters: Arc<DashMap<OscPath, OscInfo>>,
    /// Buffer that is filled with values collected from the OSC stream.
    /// If the buffer doesn't contain value it hasn't been seen since last flush.
    pub parameter_cache: Arc<DashMap<OscPath, OscType>>,
    /// The OSC server we recieve updates from
    #[serde(skip)]
    #[allow(dead_code, reason = "Keeps The threads in scope, and might be needed later")]
    osc_server: Option<OscServer>,
    /// Spawns our own OSCQuery advertising 
    #[allow(dead_code)]
    #[allow(dead_code, reason = "Keeps The threads in scope, and might be needed later")]
    query_server: Option<OscQueryServer>,
}

impl VrcInfo {
    pub fn new(global_map: Arc<Mutex<GlobalMap>>) -> Arc<Mutex<VrcInfo>> {
        let avi:Arc<RwLock<Option<Avatar>>> = Arc::new(RwLock::new(None));
        
        // Instantiate
        let vrc = VrcInfo {
            vrc_connected: false,
            osc_server: None,
            query_server: None,
            in_port: None,
            out_port: None,
            avatar: Arc::clone(&avi),
            available_parameters: Arc::new(DashMap::new()),
            parameter_cache: Arc::new(DashMap::new()),
        };
        let vrc = Arc::new(Mutex::new(vrc));
        
        // Start the thread that handles finding available vrc parameters
        start_filling_available_parameters(Arc::clone(&vrc));

        // create clone for closure
        let mut vrc_lock = vrc.lock().expect("couldn't get lock");
        let cached_parameters_rcve = Arc::clone(&vrc_lock.parameter_cache);
        // Our closure that gets called whenever an OSC message is recieved
        let on_receive = move |msg: OscMessage| {
            // remove VRC Fury tagging if needed
            let addr = remove_version(&msg.addr);
            
            // if there is a value push it to our cache.
            if let Some(arg) = msg.args.first() {
                cached_parameters_rcve.insert(OscPath(addr), arg.to_owned());
            } else {
                log::warn!("empty message recieved: {:?}", msg);
                cached_parameters_rcve.insert(OscPath(addr), OscType::Nil);
            }
        };

        //create the low-latency server.
        let recieving_port = 9001;
        let mut vrc_server = OscServer::new(recieving_port, Ipv4Addr::LOCALHOST, on_receive);
        let port_used = vrc_server.start();
        vrc_lock.in_port = Some(port_used);

        // if the server wasn't able to capture the port start advertising the port it was bound to.
        let mut osc_server = None;
        if port_used != recieving_port {
            osc_server = Some(OscQueryServer::new(recieving_port));
            osc_server.unwrap().start();
            log::warn!("Not using VRC dedicated ports, expect slower operations.");
        }

        // the callback called when each device tick starts
        let avi_refresh = Arc::clone(&avi);
        let params_refresh = Arc::clone(&vrc_lock.parameter_cache);
        let on_refresh = move |inputs: &DashMap<Id, InputNode>| {          
            // If we have an avi in use, and haptics are on the avatar we can use haptics
            let avi_option = avi_refresh.read().expect("Unable to lock avi");
            if let Some(avi_read) = &*avi_option {
                if let Some(conf) = &avi_read.conf {
                    // for each node in our config, see if we have recieved a value.
                    for node in &conf.nodes {
                        if let Some(value) = params_refresh.get(&OscPath(node.address.clone())) {
                            //create node basic's
                            let position = &node.node_data;
                            let mut in_node = InputNode::new(
                                position.to_owned(), 
                                vec![node.target_bone.to_str().to_string()],
                                Id(node.address.clone())
                            );
                            
                            // insert the value into our hashmap
                            if let Some(intensity) = value.clone().float() {
                                in_node.set_intensity(intensity);
                            } else {
                                log::error!("Couldn't find f32 value for: {:?}", position);
                                in_node.set_intensity(0.0);
                            }
                            
                            inputs.insert(Id(node.address.clone()), in_node);
                        }
                    }
                }
            }
        };

        drop(vrc_lock);

        // register callback
        let mut lock = global_map.lock().expect("couldn't get lock");
        lock.register_refresh(on_refresh);
        return vrc;
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Avatar {
    /// The avatar reffered to by the VRC api
    id: String,
    /// the name of the prefab referenced by the parameter on the avatar
    prefab_name: Option<String>,
    /// All information mapping OSC Parameters to their needed formats
    conf: Option<GameMap>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
/// Simple wrapper for the String class.
/// Represnts a full OscPath without any elements stripped,
/// other than the VRC Fury naming.
pub struct OscPath(pub String);