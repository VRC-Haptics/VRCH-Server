use super::parsing::{parse_incoming, remove_version, OscInfo};
use super::{Avatar, GameMap, MsgToMainVrc, OscPath, VrcHandle, PREFAB_PREFIX};
use crate::api::ApiManager;
use crate::vrc::AVATAR_ID_PATH;

use dashmap::DashMap;
use libloading::Library;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};

type PortCallback = unsafe extern "C" fn(u16);
type StartListener = unsafe extern "C" fn(PortCallback);
type StopListener = unsafe extern "C" fn();

static PORT_SENDER: OnceLock<Mutex<Option<mpsc::Sender<u16>>>> = OnceLock::new();

unsafe extern "C" fn dispatch_port(port: u16) {
    if let Some(lock) = PORT_SENDER.get() {
        let guard = lock.blocking_lock();
        if let Some(sender) = guard.as_ref() {
            let _ = sender.blocking_send(port);
        }
    }
}

pub async fn start_filling_available_parameters(
    vrc: VrcHandle,
    api: &'static Mutex<ApiManager>,
    params: Arc<DashMap<OscPath, OscInfo>>,
) {
    tokio::spawn(async move {
        let library_path = Path::new("./sidecars/listen-for-vrc.dll");
        let library = match unsafe { Library::new(library_path) } {
            Ok(lib) => lib,
            Err(err) => {
                log::error!("Failed to load VRC discovery library: {}", err);
                return;
            }
        };

        let start: libloading::Symbol<StartListener> =
            match unsafe { library.get(b"vrc_start_listener\0") } {
                Ok(symbol) => symbol,
                Err(err) => {
                    log::error!("Failed to load start symbol: {}", err);
                    return;
                }
            };

        let stop: libloading::Symbol<StopListener> =
            match unsafe { library.get(b"vrc_stop_listener\0") } {
                Ok(symbol) => symbol,
                Err(err) => {
                    log::error!("Failed to load stop symbol: {}", err);
                    return;
                }
            };

        let mut receiver = {
            let (tx, rx) = mpsc::channel::<u16>(2);
            let storage = PORT_SENDER.get_or_init(|| Mutex::new(None));
            let mut guard = storage.lock().await;
            *guard = Some(tx);
            rx
        };

        unsafe {
            start(dispatch_port);
        }

        while let Some(port) = receiver.recv().await {
            log::debug!("VRC discovery library reported port: {}", port);

            run_vrc_http_polling(port, &params, vrc.clone(), &api).await;

            vrc.send(MsgToMainVrc::VrcDisconnected).await;
        }

        unsafe {
            stop();
        }

        if let Some(lock) = PORT_SENDER.get() {
            let mut guard = lock.blocking_lock();

            *guard = None;
        }
    });
}

/// Fetches the HTTP response text from the given URL using a blocking reqwest client.
///
/// # Arguments
///
/// * `url` - The URL to fetch.
async fn fetch_http_response(url: &str) -> Result<String, reqwest::Error> {
    reqwest::get(url).await?.text().await
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
async fn create_avatar(
    params: &DashMap<OscPath, OscInfo>,
    new_id: String,
    api: &Mutex<ApiManager>,
) -> Avatar {
    // Attempt to load the new configuration using OSC parameters.
    let configs = load_configs(params, api).await;
    let names = configs
        .iter()
        .map(|conf| conf.meta.map_name.clone())
        .collect();
    log::info!("Updated avatar with new configuration");

    Avatar {
        id: new_id,
        prefab_names: names,
        configs: configs,
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
async fn load_configs(params: &DashMap<OscPath, OscInfo>, api: &Mutex<ApiManager>) -> Vec<GameMap> {
    let mut configs = vec![];
    if let Some(prefabs) = get_prefab_info(params) {
        for prefab in prefabs {
            let mut lock = api.try_lock().expect("Couldn't get lock");
            match lock.load_map(prefab.0, prefab.1, prefab.2).await {
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
async fn run_vrc_http_polling(
    port: u16,
    params: &DashMap<OscPath, OscInfo>,
    vrc: VrcHandle,
    api: &Mutex<ApiManager>,
) {
    let url = format!("http://127.0.0.1:{}/", port);
    log::debug!("Started polling HTTP.");

    loop {
        match fetch_http_response(&url).await {
            Ok(text) => {
                // Update OSC parameters based on the incoming HTTP response.
                let (present_parameters, new_avi) = update_params_from_text(&text, params);

                if new_avi {
                    params.retain(|key, _| present_parameters.contains(key));

                    let Some(id_path) = params.get(&OscPath(AVATAR_ID_PATH.to_string())) else {
                        log::error!("Unable to find avatar id message in vrc http response");
                        continue;
                    };

                    let mid = id_path.value.first().unwrap().clone();
                    let new_id = mid.string().unwrap();

                    let new_avatar = create_avatar(params, new_id.to_string(), &api).await;
                    vrc.send(MsgToMainVrc::FlushCache).await;
                    vrc.send(MsgToMainVrc::NewAvatar(new_avatar)).await;
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

                let num_str = parts[2].strip_prefix('v').unwrap_or("0");

                // parse the remainder as an i32
                let version = num_str
                    .parse::<u32>()
                    .unwrap_or_else(|_|{ 
                        log::error!("Could not parse verison number: {:?}", key_str);
                        0
                    });

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
