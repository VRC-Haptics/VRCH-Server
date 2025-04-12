use super::parsing::{parse_incoming, OscInfo};
use super::{Avatar, OscPath, PREFAB_PREFIX};
use crate::vrc::config::load_vrc_config;
use crate::vrc::AVATAR_ID_PATH;
use crate::VrcInfo;

use std::io::{BufRead, BufReader, ErrorKind};
use std::process::{Command, Stdio};
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
                            log::debug!("Received FOUND message with port: {}", port);
                            let mut vrc = vrc_clone.lock().expect("couldn't lock vrc");
                            let params = {
                                vrc.vrc_connected = true;
                                &Arc::clone(&vrc.available_parameters)
                            };
                            let avatar = { Arc::clone(&vrc.avatar) };
                            drop(vrc);
                            // Call the sub-function with the extracted port.
                            run_vrc_http_polling(port, params, avatar);
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

/// polls the
fn run_vrc_http_polling(
    port: u16,
    params: &DashMap<OscPath, OscInfo>,
    avatar: Arc<RwLock<Option<Avatar>>>,
) {
    let url = format!("http://127.0.0.1:{}/", port);
    log::debug!("Started polling: {}", port);

    let mut first = true;
    loop {
        // Make the HTTP request (using the blocking client for simplicity)
        match reqwest::blocking::get(&url) {
            Ok(response) => {
                if let Ok(text) = response.text() {
                    // Attempt to parse the non-standard formatted response.
                    let node_info = parse_incoming(&text);
                    // Handling this way so that we can do something when values change later
                    for node in node_info {
                        match params.get(&node.full_path) {
                            // path is being updated
                            Some(old_node) => {
                                if *old_node != node {
                                    params.insert(node.full_path.clone(), node);
                                }
                            }
                            // path is new (should only happen on starup)
                            None => {
                                if !first {
                                    log::trace!("Inserting:{:?}", node)
                                };
                                params.insert(node.full_path.clone(), node);
                            }
                        };
                    }

                    // Avi is the struct that provides OSC -> Location mapping
                    // params lists the advertised availaible parameters.
                    // check if we need to update the configuration.
                    // if we had an avatar already recorded.
                    {
                        let avi_read = avatar.read().expect("unable to get read lock");
                        if let Some(existing_avi) = &*avi_read {
                            // There is an active avatar.
                            if let Some(new_contents) =
                                params.get(&OscPath(AVATAR_ID_PATH.to_string()))
                            {
                                if let Some(new_values) = &new_contents.value {
                                    // Unwrap the new id from the OSC data.
                                    let new_id = new_values
                                        .first().unwrap()
                                        .clone()
                                        .string().unwrap();
                                    if existing_avi.id != new_id {
                                        // re-enter new data as needed.
                                    }
                                }
                            }
                            // Drop `avi_read` automatically here.
                        }
                    }

                    // If no avatar is set, update it.
                    {
                        // Acquire a write lock for modifying the avatar.
                        let mut avi_write =
                            avatar.write().expect("unable to get write lock");
                        // Check if the avatar is still None.
                        if avi_write.is_none() {
                            // Load configs from file.
                            let mut configs = vec![];
                            if let Some(prefabs) = get_prefab_info(params) {
                                for prefab in prefabs {
                                    match load_vrc_config(
                                        prefab.0,
                                        prefab.1,
                                        prefab.2,
                                        vec!["./map_configs".into()]
                                    ) {
                                        Ok(map) => configs.push(map),
                                        Err(err) => {
                                            match err.kind() {
                                                std::io::ErrorKind::NotFound => {
                                                    log::error!("Unable to load config");
                                                },
                                                other => {
                                                    log::error!("Error of kind: {}", other);
                                                },
                                            }
                                        }
                                    }
                                }
                            }

                            // Merge configs if any are loaded.
                            if let Some((first_config, rest)) = configs.split_first_mut() {
                                for conf in rest {
                                    first_config.nodes.append(&mut conf.nodes);
                                    first_config.meta.map_author += &format!("+{}", conf.meta.map_author);
                                    first_config.meta.map_name += &format!("+{}", conf.meta.map_name);
                                }

                                // Retrieve the avatar id from params.
                                let id_info = params.get(&OscPath(AVATAR_ID_PATH.to_string()))
                                    .expect("couldn't find avatar id");
                                let id = id_info.value.as_ref().unwrap().first()
                                    .unwrap().clone().string().unwrap();

                                // Create a new Avatar.
                                let new_avi = Avatar {
                                    id,
                                    prefab_name: Some(first_config.meta.map_name.clone()),
                                    conf: Some(first_config.to_owned()),
                                    // Initialize other Avatar fields as needed.
                                };

                                // Update the avatar.
                                *avi_write = Some(new_avi);
                            } else {
                                log::info!("No loaded configs for this avatar.");
                            }
                        }
                    }
                } else {
                    log::error!("Failed to read response text");
                }
            }
            Err(err) => {
                if err.is_connect() {
                    log::error!("connection to VRC HTTP failed");
                    return;
                } else {
                    log::error!("HTTP request failed: {}", err);
                }
            }
        }
        // Wait for a regular interval before the next query.
        thread::sleep(Duration::from_secs(2));
        first = false;
    }
}

/// Searches the DashMap for haptic prefabs. Paths must follow the pattern:
/// `/avatar/parameters/haptic/prefabs/<author>/<name>/<version>`
/// and returns an Option containing a vector of tuples (author, name, version).
pub fn get_prefab_info(map: &DashMap<OscPath, OscInfo>) -> Option<Vec<(String, String, u32)>> {
    let mut results = Vec::new();

    for entry in map.iter() {
        let key_str = entry.key().0.to_string();

        // Check if the key starts with the expected prefix.
        if let Some(rest) = key_str.strip_prefix(PREFAB_PREFIX) {
            // Expected remainder is "<name>/<author>/<version>".
            let parts: Vec<&str> = rest.split('/').collect();

            if parts.len() == 3 {
                if let Ok(version) = parts[2].parse::<u32>() {
                    let name = parts[1].to_string();
                    let author = parts[0].to_string();

                    // Build tuple with order: (author, name, version)
                    results.push((author, name, version));
                } else {
                    log::error!(
                        "Unable to parse version into unsigned integer: {}",
                        parts[2]
                    );
                }
            } else {
                log::warn!("Malformed Haptics Prefab info: {}", key_str);
            }
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
