use std::{collections::HashSet, path::PathBuf};
use walkdir::WalkDir;
use crate::vrc::config::GameMap;
use std::fs;

use tauri_plugin_http::reqwest::blocking::get;

pub struct ApiManager {
    pub config_folder: String,
    pub base_url: String,
    pub remote_maps: Option<Vec<NetworkAvailableMap>>,
    pub local_maps: HashSet<LocalAvailableMap>,
}

/// Represents the config files that are available for retrieval via API.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct NetworkAvailableMap {
    author: String,
    name: String,
    version: u32,
    url: String,
}

/// Represents the config files available on disk
#[derive(Debug, serde::Deserialize, serde::Serialize, Hash, PartialEq, Eq)]
pub struct LocalAvailableMap {
    author: String,
    name: String,
    version: u32,
    path: PathBuf,
}

impl ApiManager {
    /// Creates a new ApiManager.
    /// Caches WILL NOT be filled until refresh_Caches is called.
    pub fn new() -> ApiManager {

        let local_path = "./map_configs/".to_string();
        let base_url = "http://vrc-haptics.github.io/haptic-config-hosting/".to_string();

        ApiManager { 
            config_folder: local_path,
            base_url: base_url,
            remote_maps: None,
            local_maps: HashSet::new(),
        }
    }

    /// Refreshes all cache types (expensive and network blocking)
    pub fn refresh_caches(&mut self) {
        self.refresh_local_index();
        self.refresh_remote_index();

        // log refreshed values.
        log::trace!("Local Cache: {:?} Maps", self.local_maps.len()); 
        if let Some(remote) = &self.remote_maps {
            log::trace!("Remote Cache: {:?} Maps", remote.len());
        } else {
            log::error!("Empty Remote Cache");
        }
        
    }

    /// Loads the requested GameMap and returns it.
    /// Searches Local storage first, if no locally cached value is found it is retrieved 
    pub fn load_map(&mut self, author: String, name: String, version: u32) -> Result<GameMap, ApiRetrievalError> {
        // Look for local maps
        for local in self.local_maps.iter() {
            if name == local.name && author == local.author {
                if local.version == version {
                    // if we can't load the desired map refresh the index and recursively try again.
                    if let Ok(content) = fs::read_to_string(local.path.clone()) {
                        if let Ok(map) = serde_json::from_str::<GameMap>(&content) {
                            return Ok(map);
                        } else {
                            self.refresh_local_index();
                            return self.load_map(author, name, version);
                        }
                    } else {
                        self.refresh_local_index();
                        return self.load_map(author, name, version);
                    }
                } // TODO: try to resolve versions
            }
        };


        // try to retrieve remote 
        if let Some(remote_maps) = &self.remote_maps {
            for remote in remote_maps.iter() {
                if name == remote.name && author == remote.author {
                    let request_url = self.base_url.clone() + &remote.url;
                    // if we can't load the desired map refresh the index and recursively try again.
                    if let Ok(content) = get(request_url.clone()) {
                        if let Ok(map) = content.json::<GameMap>() {
                            return Ok(map);
                        } else {
                            return Err(ApiRetrievalError::BadResponseFromServer(
                                format!("Bad map recieved from server. Author:{}, name:{}, version:{}", 
                                    remote.author, remote.name, remote.version)));
                        }
                    } else {
                        return Err(ApiRetrievalError::UnableToRetrieve(format!("Error Retrieving: {}", request_url)));
                    }
                }
            }
        }

        return Err(ApiRetrievalError::MapNotFound(format!("No Map found for:  Author:{}, name:{}, version:{}", 
                                    author, name, version)));
        
    }

    /// Re-indexes the local config files.
    /// Each config file is "probably" valid, it atleast has each of the needed fields.
    pub fn refresh_local_index(&mut self) {
        // Find already locally cached maps.
        for entry in WalkDir::new(&self.config_folder).into_iter().filter_map(Result::ok) {
            if entry.file_type().is_file() {
                let content = fs::read_to_string(entry.path())
                    .expect(&format!("unable to load string from file: {:?}", &entry.clone().into_path()));
                // Deserialize JSON content into GameMap
                if let Ok(game_map) = serde_json::from_str::<GameMap>(&content) {
                    let new_value = self.local_maps.insert(LocalAvailableMap { 
                        author: game_map.meta.map_author, 
                        name: game_map.meta.map_name, 
                        version: game_map.meta.map_version,
                        path: entry.clone().into_path()
                    });

                    if !new_value {
                        log::trace!("{:?}", &self.local_maps);
                        log::warn!("Duplicate config files, Will be ignored: {:?}", entry.file_name())
                    }
                } else {
                    log::warn!("Unable to load file as config: {:?}", entry.file_name());
                }
            }
        }
    }

    /// Calls to refresh files available on the remote index.
    /// Fills self.available_maps with result.
    pub fn refresh_remote_index(&mut self) {
        let res = get(self.base_url.clone() + "catalog.json");
        let res = res.expect("Unable to fetch map index");

        log::trace!("Retrieved remote index with status: {:?}", res.status());
        let text = res.text().expect("unable to get text from index response.");
        match serde_json::from_str::<Vec<NetworkAvailableMap>>(&text) {
           Ok(updated_index) => self.remote_maps = Some(updated_index),
           Err(err) => {
            log::error!("Unable to parse returned response: {}\n{}", err, &text);
           }
        };
    }
}


#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum ApiRetrievalError {
    UnableToRetrieve(String),
    BadResponseFromServer(String),
    MapNotFound(String),
}