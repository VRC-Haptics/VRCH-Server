use super::parsing::{parse_incoming, remove_version, OscInfo};
use super::{Avatar, GameMap, OscPath, PREFAB_PREFIX};
use crate::api::ApiManager;
use crate::vrc::AVATAR_ID_PATH;
use crate::{mapping::input_node::InputType, GlobalMap, VrcInfo};

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::process::{id, Command, Stdio};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use dashmap::DashMap;

pub fn start_filling_available_parameters(
    vrc: Arc<Mutex<VrcInfo>>,
    api: Arc<Mutex<ApiManager>>,
    global_map: Arc<Mutex<GlobalMap>>,
) {
    let vrc_clone = Arc::clone(&vrc);
    thread::spawn(move || {
        // Launch the sidecar process.
        let mut child = Command::new("./sidecars/listen-for-vrc.exe")
            .arg(format!("--pid={}", id()))
            .creation_flags(0x08000000 as u32)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("Failed to launch sidecar");

        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let reader = BufReader::new(stdout);

        // Monitor the sidecar output line by line.
        for line in reader.lines() {
            match line {
                Ok(msg) => {
                    if let Some(port_str) = msg.strip_prefix("FOUND:") {
                        // Try parsing the port as a u16.
                        if let Ok(port) = port_str.trim().parse::<u16>() {
                            log::debug!("Sidecar found vrc with port: {}", port);
                            let mut vrc = vrc_clone.lock().expect("couldn't lock vrc");
                            let params = {
                                vrc.vrc_connected = true;
                                &Arc::clone(&vrc.available_parameters)
                            };
                            let avatar = { Arc::clone(&vrc.avatar) };
                            drop(vrc);
                            // Call the sub-function with the extracted port.
                            run_vrc_http_polling(
                                port,
                                params,
                                avatar,
                                Arc::clone(&vrc_clone),
                                Arc::clone(&api),
                                Arc::clone(&global_map),
                            );

                            // purge connected settings.
                            let mut vrc_lock = vrc_clone.lock().expect("COulnd't lock vrc.");
                            vrc_lock.vrc_connected = false;
                            vrc_lock.available_parameters.clear();
                            vrc_lock.purge_cache();
                            let mut avatar_lock =
                                vrc_lock.avatar.write().expect("Couldn't get read instance");
                            *avatar_lock = None;
                            // When run_vrc_http_polling returns, continue waiting for the next FOUND message.
                        } else {
                            log::error!("Error: Could not parse port from message: {}", msg);
                        }
                    } else if msg.starts_with("Attached to PID") {
                        log::trace!("sidecar process started: {}", msg);
                    } else {
                        log::error!("Received non-matching message: {}", msg);
                    }
                }
                Err(e) => {
                    log::error!("Error reading sidecar output: {}", e);
                    break;
                }
            }
        }
    });
}

/// Fetches the HTTP response text from the given URL using a blocking reqwest client.
///
/// # Arguments
///
/// * `url` - The URL to fetch.
fn fetch_http_response(url: &str) -> Result<String, reqwest::Error> {
    reqwest::blocking::get(url)?.text()
}

/// Parses the given text to extract OSC nodes and updates the provided parameters map.
///
/// # Arguments
///
/// * `text` - The HTTP response text to parse.
/// * `params` - The DashMap containing OSC parameter information.
///
/// # Returns
///
/// * (List of entries recieved, whether id has changed.)
fn update_params_from_text(
    text: &str,
    params: &DashMap<OscPath, OscInfo>,
) -> (HashSet<OscPath>, bool) {
    let mut changed = HashSet::new();
    let mut new_avi = false;

    let node_info = parse_incoming(text);
    for node in node_info {
        let raw = remove_version(&node.full_path.0);
        let path = OscPath(raw);

        changed.insert(path.clone());
        match params.get(&path) {
            Some(old_node) => {
                let should_update = *old_node != node;
                drop(old_node);
                if should_update {
                    if path.0 == AVATAR_ID_PATH {
                        new_avi = true; // value changed
                    }
                    params.insert(path, node);
                }
            }
            None => {
                if path.0 == AVATAR_ID_PATH {
                    new_avi = true; // first time we see the ID
                }
                params.insert(path, node);
            }
        }
    }

    (changed, new_avi)
}

/// Creates new avatar from available parameters.
///
/// # Arguments
///
/// * `params` - The available OSC parameters.
/// * `avatar` - The shared avatar configuration.
fn update_avatar(
    params: &DashMap<OscPath, OscInfo>,
    avatar: &Arc<RwLock<Option<Avatar>>>,
    api: Arc<Mutex<ApiManager>>,
    global_map: Arc<Mutex<GlobalMap>>,
) {
    // First, retrieve the current avatar ID (if any) using a read lock.
    let current_id = {
        let avi_read = avatar.read().expect("unable to get read lock");
        avi_read.as_ref().map(|avi| avi.id.clone())
    };

    // Extract the new avatar ID from the OSC parameters.
    if let Some(new_avi_param) = params.get(&OscPath(AVATAR_ID_PATH.to_string())) {
        let new_id = &new_avi_param
            .value
            .first()
            .unwrap()
            .clone()
            .string()
            .unwrap();

        // Compare the new id with the current avatar's id.
        if current_id.as_deref() != Some(&new_id) {
            log::info!("Avatar ID changed: {:?} -> {}", current_id, new_id);
            // Clear all InputNodes with the tag "vrc_config_node"
            if let Ok(map) = global_map.lock() {
                map.remove_all_with_tag(&"vrc_config_node".to_string());
            } else {
                log::error!("Failed to lock global_map for clearing vrc_config_node nodes");
            }

            // Attempt to load the new configuration using OSC parameters.
            let configs = load_configs(params, api);
            log::info!("Updated avatar with new configuration {configs:?}");

            push_to_global_map(&configs, global_map);
            let mut avi_write = avatar.write().expect("unable to get write lock");
            if let Some(avi_mut) = avi_write.as_mut() {
                let names = configs
                    .iter()
                    .map(|conf| conf.meta.map_name.clone())
                    .collect();
                avi_mut.id = new_id.to_string();
                avi_mut.configs = configs;
                avi_mut.prefab_names = names;
            } else {
                let names = configs
                    .iter()
                    .map(|conf| conf.meta.map_name.clone())
                    .collect();
                let new_avi = Avatar {
                    id: new_id.to_string(),
                    configs: configs,
                    prefab_names: names,
                };
                *avi_write = Some(new_avi);
            };
        }
    }
}

fn push_to_global_map(configs: &Vec<GameMap>, global_map: Arc<Mutex<GlobalMap>>) {
    let lock = global_map.lock().unwrap();
    for conf in configs {
        for node in &conf.nodes {
            let mut haptic_node = node.node_data.clone();
            // if external address apply all tag.
            if node.is_external_address {
                haptic_node.groups.push(crate::mapping::NodeGroup::All);
            }

            let mut input_type = InputType::INTERP;
            if node.is_external_address {
                input_type = InputType::ADDITIVE
            }

            // set intensity and push to map.
            let err = lock.add_input_node(
                haptic_node,
                vec![
                    format!(
                        "{}_{}_{}",
                        conf.meta.map_author, conf.meta.map_name, conf.meta.map_version
                    ),
                    node.target_bone.to_string(),
                    "vrc_config_node".to_string(),
                ],
                node.address.clone(),
                node.radius,
                Some(input_type),
            );
            if err.is_err() {
                log::error!(
                    "Error: {:?} inserting Config: {} node: {}",
                    err,
                    conf.meta.map_name,
                    node.address
                );
            }
        }
    }
}

/// Loads configuration files from disk (using OSC parameters for prefab info) and merges them.
///
/// # Arguments
///
/// * `params` - The OSC parameters containing prefab information.
///
/// # Returns
///
/// * `Some(GameMap)` if configurations were successfully loaded and merged.
/// * `None` if no configs were found or loaded.
fn load_configs(params: &DashMap<OscPath, OscInfo>, api: Arc<Mutex<ApiManager>>) -> Vec<GameMap> {
    let mut configs = vec![];
    if let Some(prefabs) = get_prefab_info(params) {
        for prefab in prefabs {
            let mut lock = api.lock().expect("Unable to obtain api lock");
            match lock.load_map(prefab.0, prefab.1, prefab.2) {
                Ok(map) => configs.push(map),
                Err(err) => match err {
                    other => {
                        log::error!("Error loading config: {:?}", other);
                    }
                },
            }
        }
    } else {
        log::trace!("No prefab info");
    }
    configs
}

/// ---------------------------------------------------------------------
/// Main Polling Loop
/// ---------------------------------------------------------------------

/// Continuously polls the VRC HTTP endpoint for OSC parameters and updates both
/// the parameter map and the avatar configuration accordingly.
///
/// # Arguments
///
/// * `port` - The port on which the VRC HTTP server is running.
/// * `params` - A reference to the DashMap holding OSC parameter data.
/// * `avatar` - A shared, thread-safe reference to the current avatar configuration.
fn run_vrc_http_polling(
    port: u16,
    params: &DashMap<OscPath, OscInfo>,
    avatar: Arc<RwLock<Option<Avatar>>>,
    vrc: Arc<Mutex<VrcInfo>>,
    api: Arc<Mutex<ApiManager>>,
    global_map: Arc<Mutex<GlobalMap>>,
) {
    let url = format!("http://127.0.0.1:{}/", port);
    log::debug!("Started polling HTTP.");

    loop {
        match fetch_http_response(&url) {
            Ok(text) => {
                // Update OSC parameters based on the incoming HTTP response.
                let (present_parameters, new_avi) = update_params_from_text(&text, params);

                // remove all old parameters (not present)
                if new_avi {
                    params.retain(|key, _| present_parameters.contains(key));

                    {
                        let mut vrc_lock = vrc.lock().expect("couldn't lock vrc");
                        vrc_lock.purge_cache();
                    }

                    update_avatar(params, &avatar, Arc::clone(&api), Arc::clone(&global_map));
                }
            }
            Err(err) => {
                if err.is_connect() {
                    log::error!("Connection to VRC HTTP failed");
                    return;
                } else {
                    log::error!("HTTP request failed: {}", err);
                }
            }
        }
        // Sleep before the next polling iteration.
        thread::sleep(Duration::from_secs(2));
    }
}

/// Searches the DashMap for haptic prefabs. Paths must follow the pattern:
/// `/avatar/parameters/haptic/prefabs/<author>/<name>/v<version>`
/// and returns an Option containing a vector of tuples (author, name, version).
pub fn get_prefab_info(map: &DashMap<OscPath, OscInfo>) -> Option<Vec<(String, String, u32)>> {
    let mut results = Vec::new();

    for entry in map.iter() {
        let key_str = entry.key().0.to_string();

        // Check if the key starts with the expected prefix.
        if let Some(rest) = key_str.strip_prefix(PREFAB_PREFIX) {
            // Expected remainder is "<name>/<author>" with version as the value
            let parts: Vec<&str> = rest.split('/').collect();

            if parts.len() == 3 {
                let name = parts[1].to_string();
                let author = parts[0].to_string();

                let num_str = parts[2].strip_prefix('v').expect("no v in version entry.");

                // parse the remainder as an i32
                let version = num_str
                    .parse::<u32>()
                    .expect(&format!("Could not parse verison number: {:?}", key_str));

                // sometimes I hate this language
                log::info!("Avatar has prefab: {:?}", (&author, &name, &version));
                results.push((author, name, version));
            } // could be malformed, but probably just partial path.
        }
    }

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}
