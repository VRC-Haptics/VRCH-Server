use crate::vrc::config::GameMap;
use std::fs;
use std::sync::{Arc, Mutex};
use std::{collections::HashSet, path::PathBuf};
use walkdir::WalkDir;

use tauri_plugin_http::reqwest::blocking::get;

pub struct ApiManager {
    pub config_folder: String,
    pub base_url: String,
    pub remote_maps: Arc<Mutex<Option<Vec<NetworkAvailableMap>>>>,
    pub local_maps: Arc<Mutex<HashSet<LocalAvailableMap>>>,
    refresh_handle: Option<std::thread::JoinHandle<()>>,
}

/// Represents the config files that are available for retrieval via API.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkAvailableMap {
    author: String,
    name: String,
    version: u32,
    url: String,
}

/// Represents the config files available on disk
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Hash, PartialEq, Eq)]
pub struct LocalAvailableMap {
    author: String,
    name: String,
    version: u32,
    path: PathBuf,
}

impl ApiManager {
    /// Creates a new ApiManager.
    /// Caches WILL NOT be filled until refresh_caches is called.
    pub fn new() -> ApiManager {
        let local_path = "./map_configs/".to_string();
        let base_url = "http://vrc-haptics.github.io/haptic-config-hosting/".to_string();

        ApiManager {
            config_folder: local_path,
            base_url,
            remote_maps: Arc::new(Mutex::new(None)),
            local_maps: Arc::new(Mutex::new(HashSet::new())),
            refresh_handle: None,
        }
    }

    /// Refreshes all caches asynchronously in a separate thread.
    /// Returns immediately. Check is_refreshing() to see if refresh is still in progress, or `self.wait_for_refresh()` to block until completed
    pub fn refresh_caches(&mut self) {
        // If a refresh is already in progress, don't start another one
        if self.is_refreshing() {
            log::debug!("Cache refresh already in progress, skipping new refresh");
            return;
        }

        let config_folder = self.config_folder.clone();
        let base_url = self.base_url.clone();
        let local_maps = Arc::clone(&self.local_maps);
        let remote_maps = Arc::clone(&self.remote_maps);

        let handle = std::thread::spawn(move || {
            log::debug!("Starting async cache refresh");

            // Refresh local index
            Self::refresh_local_index_thread(config_folder, local_maps.clone());

            // Refresh remote index
            Self::refresh_remote_index_thread(base_url, remote_maps.clone());

            // Log refreshed values
            if let Ok(local) = local_maps.lock() {
                log::trace!("Local Cache: {:?} Maps", local.len());
            }

            if let Ok(remote) = remote_maps.lock() {
                if let Some(ref maps) = *remote {
                    log::trace!("Remote Cache: {:?} Maps", maps.len());
                } else {
                    log::error!("Empty Remote Cache");
                }
            }

            log::debug!("Async cache refresh completed");
        });

        self.refresh_handle = Some(handle);
    }

    /// Checks if a cache refresh is currently in progress
    pub fn is_refreshing(&self) -> bool {
        self.refresh_handle
            .as_ref()
            .map(|h| !h.is_finished())
            .unwrap_or(false)
    }

    /// Waits for any ongoing cache refresh to complete
    pub fn wait_for_refresh(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(handle) = self.refresh_handle.take() {
            handle.join().map_err(|_| "Thread panicked")?;
        }
        Ok(())
    }

    /// Refreshes all cache types (expensive and network blocking)
    pub fn refresh_caches_blocking(&mut self) {
        self.refresh_local_index();
        self.refresh_remote_index();

        // log refreshed values.
        if let Ok(local) = self.local_maps.lock() {
            log::trace!("Local Cache: {:?} Maps", local.len());
        }

        if let Ok(remote) = self.remote_maps.lock() {
            if let Some(ref maps) = *remote {
                log::trace!("Remote Cache: {:?} Maps", maps.len());
            } else {
                log::error!("Empty Remote Cache");
            }
        }
    }

    /// Thread-safe version of refresh_local_index for use in async refresh
    fn refresh_local_index_thread(
        config_folder: String,
        local_maps: Arc<Mutex<HashSet<LocalAvailableMap>>>,
    ) {
        let mut new_local_maps = HashSet::new();

        for entry in WalkDir::new(&config_folder)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.file_type().is_file() {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Ok(game_map) = serde_json::from_str::<GameMap>(&content) {
                        let map = LocalAvailableMap {
                            author: game_map.meta.map_author,
                            name: game_map.meta.map_name,
                            version: game_map.meta.map_version,
                            path: entry.clone().into_path(),
                        };

                        if !new_local_maps.insert(map) {
                            log::warn!(
                                "Duplicate config files, Will be ignored: {:?}",
                                entry.file_name()
                            );
                        }
                    } else {
                        log::warn!("Unable to load file as config: {:?}", entry.file_name());
                    }
                } else {
                    log::warn!("Unable to read file: {:?}", entry.path());
                }
            }
        }

        // Update the shared state
        if let Ok(mut maps) = local_maps.lock() {
            *maps = new_local_maps;
        }
    }

    /// Thread-safe version of refresh_remote_index for use in async refresh
    fn refresh_remote_index_thread(
        base_url: String,
        remote_maps: Arc<Mutex<Option<Vec<NetworkAvailableMap>>>>,
    ) {
        match get(base_url.clone() + "catalog.json") {
            Ok(res) => {
                log::trace!("Retrieved remote index with status: {:?}", res.status());
                match res.text() {
                    Ok(text) => match serde_json::from_str::<Vec<NetworkAvailableMap>>(&text) {
                        Ok(updated_index) => {
                            if let Ok(mut maps) = remote_maps.lock() {
                                *maps = Some(updated_index);
                            }
                        }
                        Err(err) => {
                            log::error!("Unable to parse returned response: {}\n{}", err, &text);
                        }
                    },
                    Err(err) => {
                        log::error!("Unable to get text from index response: {}", err);
                    }
                }
            }
            Err(err) => {
                log::error!("Unable to fetch map index: {}", err);
            }
        }
    }

    // Loads the requested GameMap and returns it.
    /// Searches Local storage first, if no locally cached value is found it is retrieved
    pub fn load_map(
        &mut self,
        author: String,
        name: String,
        version: u32,
    ) -> Result<GameMap, ApiRetrievalError> {
        // Look for local maps
        let should_refresh = {
            if let Ok(local_maps) = self.local_maps.lock() {
                for local in local_maps.iter() {
                    if name == local.name && author == local.author {
                        if local.version == version {
                            // if we can't load the desired map refresh the index and recursively try again.
                            if let Ok(content) = fs::read_to_string(&local.path) {
                                if let Ok(map) = serde_json::from_str::<GameMap>(&content) {
                                    return Ok(map);
                                } else {
                                    return Err(ApiRetrievalError::BadResponseFromServer(format!(
                                        "Failed to parse local map file: {:?}",
                                        local.path
                                    )));
                                }
                            } else {
                                return Err(ApiRetrievalError::UnableToRetrieve(format!(
                                    "Failed to read local map file: {:?}",
                                    local.path
                                )));
                            }
                        } // TODO: try to resolve versions
                    }
                }
                false
            } else {
                true // If we can't lock, trigger a refresh
            }
        }; // Lock is dropped here

        if should_refresh {
            self.refresh_local_index();
            // Try once more after refresh
            if let Ok(local_maps) = self.local_maps.lock() {
                for local in local_maps.iter() {
                    if name == local.name && author == local.author && local.version == version {
                        if let Ok(content) = fs::read_to_string(&local.path) {
                            if let Ok(map) = serde_json::from_str::<GameMap>(&content) {
                                return Ok(map);
                            }
                        }
                    }
                }
            }
        }

        // try to retrieve remote
        if let Ok(remote_maps_guard) = self.remote_maps.lock() {
            if let Some(ref remote_maps) = *remote_maps_guard {
                for remote in remote_maps.iter() {
                    if name == remote.name && author == remote.author {
                        let request_url = self.base_url.clone() + &remote.url;
                        // if we can't load the desired map refresh the index and recursively try again.
                        if let Ok(content) = get(&request_url) {
                            if let Ok(map) = content.json::<GameMap>() {
                                return Ok(map);
                            } else {
                                return Err(ApiRetrievalError::BadResponseFromServer(format!(
                                    "Bad map received from server. Author:{}, name:{}, version:{}",
                                    remote.author, remote.name, remote.version
                                )));
                            }
                        } else {
                            return Err(ApiRetrievalError::UnableToRetrieve(format!(
                                "Error Retrieving: {}",
                                request_url
                            )));
                        }
                    }
                }
            }
        }

        Err(ApiRetrievalError::MapNotFound(format!(
            "No Map found for:  Author:{}, name:{}, version:{}",
            author, name, version
        )))
    }

    /// Re-indexes the local config files.
    /// Each config file is "probably" valid, it atleast has each of the needed fields.
    pub fn refresh_local_index(&mut self) {
        // Find already locally cached maps.
        let mut new_local_maps = HashSet::new();

        for entry in WalkDir::new(&self.config_folder)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.file_type().is_file() {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    // Deserialize JSON content into GameMap
                    if let Ok(game_map) = serde_json::from_str::<GameMap>(&content) {
                        let map = LocalAvailableMap {
                            author: game_map.meta.map_author,
                            name: game_map.meta.map_name,
                            version: game_map.meta.map_version,
                            path: entry.clone().into_path(),
                        };

                        if !new_local_maps.insert(map) {
                            log::trace!("{:?}", &new_local_maps);
                            log::warn!(
                                "Duplicate config files, Will be ignored: {:?}",
                                entry.file_name()
                            );
                        }
                    } else {
                        log::warn!("Unable to load file as config: {:?}", entry.file_name());
                    }
                } else {
                    log::warn!("Unable to load string from file: {:?}", entry.path());
                }
            }
        }

        if let Ok(mut maps) = self.local_maps.lock() {
            *maps = new_local_maps;
        }
    }

    /// Calls to refresh files available on the remote index.
    /// Fills self.available_maps with result.
    pub fn refresh_remote_index(&mut self) {
        match get(self.base_url.clone() + "catalog.json") {
            Ok(res) => {
                log::trace!("Retrieved remote index with status: {:?}", res.status());
                match res.text() {
                    Ok(text) => match serde_json::from_str::<Vec<NetworkAvailableMap>>(&text) {
                        Ok(updated_index) => {
                            if let Ok(mut maps) = self.remote_maps.lock() {
                                *maps = Some(updated_index);
                            }
                        }
                        Err(err) => {
                            log::error!("Unable to parse returned response: {}\n{}", err, &text);
                        }
                    },
                    Err(err) => {
                        log::error!("Unable to get text from index response: {}", err);
                    }
                }
            }
            Err(err) => {
                log::error!("Unable to fetch map index: {}", err);
            }
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum ApiRetrievalError {
    UnableToRetrieve(String),
    BadResponseFromServer(String),
    MapNotFound(String),
}
