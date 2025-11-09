pub mod cache_node;
pub mod config;
pub mod discovery;
pub mod osc_query;
pub mod parsing;

// crate dependencies
use crate::api::ApiManager;
use crate::mapping::input_node::InputType;
use crate::mapping::{global_map::StandardMenu, input_node::InputNode, Id};
use crate::osc::server::OscServer;
use crate::vrc::parsing::OscInfo;
use crate::{get_store_field, GlobalMap};

// module dependencies
use cache_node::CacheNode;
use config::GameMap;
use dashmap::DashMap;
use discovery::start_filling_available_parameters;
use osc_query::OscQueryServer;
use parsing::remove_version;

use rosc::OscMessage;
use std::time::Duration;
use std::{
    net::Ipv4Addr,
    sync::{Arc, Mutex, RwLock},
};

// "/avatar/parameters/haptic/prefabs/<author>/<name>/<version>"
// I think having trailing "/" references the contents of the path, not all the children paths.
pub const PREFAB_PREFIX: &str = "/avatar/parameters/haptic/prefabs/";
pub const INTENSITY_PATH: &str = "/avatar/parameters/haptic/global/intensity";
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
    /// NOTE: The values actual values contained in this struct are out of date by up to 2 seconds.
    pub available_parameters: Arc<DashMap<OscPath, OscInfo>>,
    /// Buffer that is filled with values collected from the OSC stream.
    /// If the buffer doesn't contain value it hasn't been seen since last flush.
    pub parameter_cache: Arc<DashMap<OscPath, CacheNode>>,
    /// Number of value entries to keep around for each `self.parameter_cache` entry
    ///
    /// VRC Refreshes at 10hz max, so 10*seconds should work just fine.
    pub cache_length: usize,
    /// How much weight distance has, 1-`dist_weight` = the velocity weight
    pub dist_weight: f32,
    /// the magic velocity multiplier. 1 is reasonable, if fast.
    pub vel_multiplier: f32,
    /// The OSC server we recieve updates from
    #[serde(skip)]
    #[allow(
        dead_code,
        reason = "Keeps The threads in scope, and might be needed later"
    )]
    osc_server: Option<OscServer>,
    /// Spawns our own OSCQuery advertising
    #[allow(
        dead_code,
        reason = "Keeps The threads in scope, and might be needed later"
    )]
    query_server: Option<OscQueryServer>,
}

impl VrcInfo {
    pub fn new(
        global_map: Arc<Mutex<GlobalMap>>,
        api: Arc<Mutex<ApiManager>>,
        app_handle: &tauri::AppHandle,
    ) -> Arc<Mutex<VrcInfo>> {
        let avi: Arc<RwLock<Option<Avatar>>> = Arc::new(RwLock::new(None));
        let value_cache_size = 10;

        let dist_weight = get_store_field(app_handle, "distance_weight").or(Some(0.20));
        let vel_multiplier = get_store_field(app_handle, "velocity_multiplier").or(Some(1.0));

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
            cache_length: value_cache_size,
            dist_weight: dist_weight.unwrap(),
            vel_multiplier: vel_multiplier.unwrap(),
        };
        let vrc = Arc::new(Mutex::new(vrc));

        // Start the thread that handles finding available vrc parameters
        start_filling_available_parameters(Arc::clone(&vrc), api, Arc::clone(&global_map));
        // create clone for closure
        let vrc_lock = vrc.lock().unwrap();
        let cached_parameters_rcve = Arc::clone(&vrc_lock.parameter_cache);
        let default_clone = vrc_lock.cache_length.clone();
        drop(vrc_lock);
        // Our closure that gets called whenever an OSC message is recieved
        let on_receive = move |msg: OscMessage| {
            // remove VRC Fury tagging if needed
            let addr = remove_version(&msg.addr);

            // if there is a value push it to our cache.
            if let Some(arg) = msg.args.first() {
                // if we have a cache going otherwise build it.
                if let Some(mut cache) = cached_parameters_rcve.get_mut(&OscPath(addr.clone())) {
                    let _ = cache.update(arg.to_owned());
                } else {
                    cached_parameters_rcve.insert(
                        OscPath(addr),
                        CacheNode::new(
                            arg.to_owned(),
                            default_clone,
                            Duration::from_secs_f32(0.12),
                            0.2,
                            1.0,
                            1.0,
                        ),
                    );
                }
            } else {
                log::warn!("empty message recieved: {:?}", msg);
            }
        };

        //create the low-latency server.
        let recieving_port = 9001;
        let mut vrc_server = OscServer::new(recieving_port, Ipv4Addr::LOCALHOST, on_receive);
        let port_used = vrc_server.start();
        let mut vrc_lock = vrc.lock().unwrap();
        vrc_lock.in_port = Some(port_used);

        // if the server wasn't able to capture the port start advertising the port it was bound to.
        if port_used != recieving_port {
            let mut osc_server = OscQueryServer::new(recieving_port);
            osc_server.start();
            log::warn!("Not using VRC dedicated ports, expect slower operations.");
        }

        // the callback called when each device tick starts
        let avi_refresh = Arc::clone(&avi);
        let params_refresh = Arc::clone(&vrc_lock.parameter_cache);
        let vrc_refresh = Arc::clone(&vrc);
        let on_refresh = move |inputs: &DashMap<Id, InputNode>, menu: &Mutex<StandardMenu>| {
            // If we have an avi in use, and haptics are on the avatar,
            // we can use haptics
            let avi_option = avi_refresh.read().expect("Unable to lock avi");
            if let Some(avi_read) = &*avi_option {
                if !&avi_read.configs.is_empty() {
                    // update menu items if we have something to dupate them with
                    let mut menu_l = menu.lock().expect("couldn't lock the menu");
                    if let Some(intensity) = params_refresh.get(&OscPath(INTENSITY_PATH.to_owned()))
                    {
                        let intensity = intensity.value().clone();
                        let intensity = intensity.raw_last();
                        if intensity > 0.001 {
                            menu_l.intensity = intensity;
                            menu_l.enable = true;
                        } else {
                            menu_l.intensity = intensity;
                            menu_l.enable = false;
                        }
                    }

                    // for each node in our configs, see if we have received a value.
                    let vrc_lock = vrc_refresh.lock().unwrap();
                    for conf in &avi_read.configs {
                        for node in &conf.nodes {
                            if let Some(mut cache_node) =
                                params_refresh.get_mut(&OscPath(node.address.clone()))
                            {
                                // update node if already created
                                if let Some(mut old_node) =
                                    inputs.get_mut(&Id(node.address.clone()))
                                {
                                    // don't do velocity for external addresses
                                    if node.is_external_address {
                                        old_node.set_intensity(cache_node.raw_last());
                                        continue;
                                    }
                                    // insert the value into the game map
                                    cache_node.set_position_weight(vrc_lock.dist_weight);
                                    cache_node.set_velocity_mult(vrc_lock.vel_multiplier);
                                    cache_node.set_contact_scale(1.0);
                                    old_node.set_intensity(cache_node.latest());
                                    continue;
                                }
                            }
                        } // for loop
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

    /// Purges the parameter cache.
    pub fn purge_cache(&mut self) {
        self.parameter_cache.clear();
        log::info!("Purged Parameter cache.");
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
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
