use super::parsing::{parse_incoming, remove_version, OscInfo};
use super::{Avatar, GameMap, OscPath, PREFAB_PREFIX};
use crate::vrc::config::load_vrc_config;
use crate::vrc::AVATAR_ID_PATH;
use crate::VrcInfo;

use std::io::{BufRead, BufReader, ErrorKind};
use std::process::{id, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use dashmap::DashMap;
use oyasumivr_oscquery;
use oyasumivr_oscquery::{OSCMethod, OSCMethodAccessType};
use serde;

pub fn start_filling_available_parameters(vrc: Arc<Mutex<VrcInfo>>) {
    let vrc_clone = Arc::clone(&vrc);
    thread::spawn(move || {
        // Launch the sidecar process.
        let mut child = Command::new("./sidecars/listen-for-vrc.exe")
            .arg(format!("--pid={}", id()))
            // Do not attach a terminal to the sidecar.
            .stdin(Stdio::null())
            // Capture its stdout so we can read the FOUND messages.
            .stdout(Stdio::piped())
            // Optionally inherit stderr to see error messages.
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
                            run_vrc_http_polling(port, params, avatar, Arc::clone(&vrc_clone));
                            // When run_vrc_http_polling returns, continue waiting for the next FOUND message.
                        } else {
                            log::error!("Error: Could not parse port from message: {}", msg);
                        }
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
fn update_params_from_text(text: &str, params: &DashMap<OscPath, OscInfo>) {
    let node_info = parse_incoming(text);
    for node in node_info {
        let path = remove_version(&node.full_path.0);
        match params.get(&OscPath(path.clone())) {
            // If the path exists and the data has changed, update it.
            Some(old_node) => {
                // Do the comparison and store the result.
                let should_update = *old_node != node;
                // Explicitly drop the guard before calling insert.
                drop(old_node);
                if should_update {
                    params.insert(OscPath(path), node);
                }
            }
            None => {
                params.insert(OscPath(path), node);
            }
        }
    }
}

/// Checks for an already active avatar and updates it if its ID does not match the OSC parameter.
///
/// # Arguments
///
/// * `params` - The OSC parameters.
/// * `avatar` - The shared avatar configuration.
fn update_existing_avatar(
    params: &DashMap<OscPath, OscInfo>,
    avatar: &Arc<RwLock<Option<Avatar>>>,
    vrc: &Mutex<VrcInfo>,
) {
    // First, retrieve the current avatar ID (if any) using a read lock.
    let current_id = {
        let avi_read = avatar.read().expect("unable to get read lock");
        avi_read.as_ref().map(|avi| avi.id.clone())
    };

    // Extract the new avatar ID from the OSC parameters.
    if let Some(new_contents) = params.get(&OscPath(AVATAR_ID_PATH.to_string())) {
        if let Some(new_values) = &new_contents.value {
            // Unwrap the new id from the OSC data.
            let new_id = new_values.first().unwrap().clone().string().unwrap();

            // Compare the new id with the current avatar's id.
            if current_id.as_deref() != Some(&new_id) {
                log::info!("Avatar ID changed: {:?} -> {}", current_id, new_id);
                let lock = vrc.lock().expect("couldn't lock vrc");
                lock.purge_cache();
                drop(lock);
                // Attempt to load the new configuration using OSC parameters.
                if let Some(new_config) = load_and_merge_configs(params) {
                    //log::trace!("new config: {:?}", new_config.nodes.len());
                    let mut avi_write = avatar.write().expect("unable to get write lock");
                    if let Some(avi_mut) = avi_write.as_mut() {
                        avi_mut.id = new_id;
                        avi_mut.conf = Some(new_config.clone());
                        avi_mut.prefab_name = Some(new_config.meta.map_name.clone());
                    } else {
                        let new_avi = Avatar {
                            id: new_id,
                            conf: Some(new_config.clone()),
                            prefab_name: Some(new_config.meta.map_name.clone()),
                        };
                        *avi_write = Some(new_avi);
                    };
                    log::info!("Updated avatar with new configuration.");
                } else {
                    // we don't have enough information to do haptics.
                    // put shell avatar together.
                    let mut avi_write = avatar.write().expect("unable to get write lock");
                    let new_avi = Avatar {
                        id: new_id,
                        conf: None,
                        prefab_name: None,
                    };
                    *avi_write = Some(new_avi);
                    log::error!("Unable to load haptics for this avatar.");
                }
            }
        }
    } else {
        log::error!("Unable to find ID parameter");
        log::info!("PARAMS: \n{:?}", params);
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
fn load_and_merge_configs(params: &DashMap<OscPath, OscInfo>) -> Option<GameMap> {
    let mut configs = vec![];
    if let Some(prefabs) = get_prefab_info(params) {
        for prefab in prefabs {
            match load_vrc_config(prefab.0, prefab.1, prefab.2, vec!["./map_configs/".into()]) {
                Ok(map) => configs.push(map),
                Err(err) => match err.kind() {
                    ErrorKind::NotFound => {
                        log::error!("Unable to load config: not found");
                    }
                    other => {
                        log::error!("Error loading config: {:?}", other);
                    }
                },
            }
        }
    } else {
        log::trace!("No prefab info");
    }
    if let Some((first_config, rest)) = configs.split_first_mut() {
        for conf in rest {
            first_config.nodes.append(&mut conf.nodes);
            first_config.meta.map_author += &format!("+{}", conf.meta.map_author);
            first_config.meta.map_name += &format!("+{}", conf.meta.map_name);
        }
        Some(first_config.to_owned())
    } else {
        None
    }
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
) {
    let url = format!("http://127.0.0.1:{}/", port);
    log::debug!("Started polling HTTP.");

    loop {
        match fetch_http_response(&url) {
            Ok(text) => {
                // Update OSC parameters based on the incoming HTTP response.
                update_params_from_text(&text, params);

                // Check for updates if an avatar is already active.
                update_existing_avatar(params, &avatar, &vrc);

                // Initialize the avatar if it hasn't been set yet.
                //initialize_avatar(params, &avatar);
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

/// Handles advertising our server for vrc to send values to if we need it.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct OscQueryServer {
    recv_port: u16,
    #[serde(skip)]
    stop_sender: Option<mpsc::Sender<()>>,
}

impl OscQueryServer {
    pub fn new(recieving_port: u16) -> Self {
        OscQueryServer {
            recv_port: recieving_port,
            stop_sender: None,
        }
    }

    pub fn start(&mut self) {
        let (tx, rx) = mpsc::channel();
        let in_port = self.recv_port.clone();
        self.stop_sender = Some(tx);

        thread::spawn(move || {
            log::debug!("Spawned VRC Advertising on port:{}", in_port);
            let tk_rt = tokio::runtime::Runtime::new().unwrap();
            tk_rt.block_on(async {
                // Initialize the OSCQuery server
                log::debug!("In port: {}", in_port);
                let (host, port) = oyasumivr_oscquery::server::init(
                    "VRC Haptics", // The name of your application (Shows in VRChat's UI)
                    in_port,
                    "./sidecars/vrc-sidecar.exe", // The (relative) path to the MDNS sidecar executable
                )
                .await
                .unwrap();
                let addr = format!("{}:{}", host, port);
                log::debug!("OscQuery on: {}", addr);
                oyasumivr_oscquery::server::add_osc_method(OSCMethod {
                    description: Some("Haptics Specific Parameters".to_string()),
                    address: "/avatar/parameters/*".to_string(),
                    ad_type: OSCMethodAccessType::Write,
                    value_type: None,
                    value: None,
                })
                .await; // /avatar/*, /avatar/parameters/*, etc.
                oyasumivr_oscquery::server::advertise().await.unwrap();
            });

            loop {
                // Check for stop signal
                if let Ok(_) = rx.try_recv() {
                    tk_rt.block_on(async {
                        let _ = oyasumivr_oscquery::server::deinit().await;
                    });
                    break;
                }
            }
        });
    }

    #[allow(dead_code)] // TODO: send deinit
    pub fn stop(&mut self) {
        if let Some(sender) = self.stop_sender.take() {
            let _ = sender.send(());
        }
    }
}
