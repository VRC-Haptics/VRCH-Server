pub mod cam_schema;

use crate::vrc::config::GameMap;
use std::fs;
use std::sync::Arc;
use std::{collections::HashSet, path::PathBuf};
use tokio::sync::Mutex;
use walkdir::WalkDir;
use crate::network::fetch_text;

pub use cam_schema::{
    CamField, CamType, CamConfig, LocalAvailableSchema, SchemaError,
};

/// The folder under the config cache that holds camera schema files.
pub const SCHEMA_FOLDER: &str = "cameras";

#[derive(Debug)]
pub struct ApiManager {
    pub config_folder: PathBuf,
    pub cam_folder: PathBuf,
    pub base_url: String,
    pub remote_maps: Arc<Mutex<Option<Vec<NetworkAvailableMap>>>>,
    pub local_maps: Arc<Mutex<HashSet<LocalAvailableMap>>>,
    pub local_cams: Arc<Mutex<HashSet<LocalAvailableSchema>>>,
    refresh_handle: Option<tokio::task::JoinHandle<()>>,
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
    ///
    /// The camera schemas live in the `cameras` folder under the cache.
    pub fn new(cache: PathBuf) -> ApiManager {
        let schemas = cache.join(SCHEMA_FOLDER);
        Self::new_with_schemas(cache, schemas)
    }

    /// Creates a new ApiManager with a named schema folder.
    pub fn new_with_schemas(cache: PathBuf, schema_folder: PathBuf) -> ApiManager {
        let base_url = "http://vrc-haptics.github.io/haptic-config-hosting/".to_string();

        ApiManager {
            config_folder: cache,
            cam_folder: schema_folder,
            base_url,
            remote_maps: Arc::new(Mutex::new(None)),
            local_maps: Arc::new(Mutex::new(HashSet::new())),
            local_cams: Arc::new(Mutex::new(HashSet::new())),
            refresh_handle: None,
        }
    }

    /// Refreshes all caches asynchronously in a separate thread.
    /// Returns immediately. Check is_refreshing() to see if refresh is still in progress, or `self.wait_for_refresh()` to block until completed
    pub async fn refresh_caches(&mut self) {
        // If a refresh is already in progress, don't start another one
        if self.is_refreshing() {
            log::debug!("Cache refresh already in progress, skipping new refresh");
            return;
        }

        let config_folder = self.config_folder.clone();
        let schema_folder = self.cam_folder.clone();
        let base_url = self.base_url.clone();
        let local_maps = Arc::clone(&self.local_maps);
        let remote_maps = Arc::clone(&self.remote_maps);
        let local_schemas = Arc::clone(&self.local_cams);

        let handle = tokio::spawn(async move {
            log::debug!("Starting async cache refresh");

            // Refresh local index
            Self::refresh_local_index_thread(
                config_folder,
                schema_folder.clone(),
                local_maps.clone(),
            )
            .await;

            // Refresh the camera schema index
            Self::refresh_schema_index_thread(schema_folder, local_schemas.clone()).await;

            // Refresh remote index
            Self::refresh_remote_index_thread(base_url, remote_maps.clone()).await;

            // Log refreshed values
            let local = local_maps.lock().await;
            log::trace!("Local Cache: {:?} Maps", local.len());

            let schemas = local_schemas.lock().await;
            log::trace!("Schema Cache: {:?} Schemas", schemas.len());

            let remote = remote_maps.lock().await;
            if let Some(ref maps) = *remote {
                log::trace!("Remote Cache: {:?} Maps", maps.len());
            } else {
                log::error!("Empty Remote Cache");
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
    pub async fn wait_for_refresh(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(handle) = self.refresh_handle.take() {
            handle.await.map_err(|_| "Thread panicked")?;
        }
        Ok(())
    }

    /// Refreshes all cache types (expensive and network blocking)
    pub async fn refresh_caches_blocking(&mut self) {
        self.refresh_local_index().await;
        self.refresh_schema_index().await;
        self.refresh_remote_index().await;

        // log refreshed values.
        let local = self.local_maps.lock().await;
        log::trace!("Local Cache: {:?} Maps", local.len());

        let schemas = self.local_cams.lock().await;
        log::trace!("Schema Cache: {:?} Schemas", schemas.len());

        let remote = self.remote_maps.lock().await;
        if let Some(ref maps) = *remote {
            log::trace!("Remote Cache: {:?} Maps", maps.len());
        } else {
            log::error!("Empty Remote Cache");
        }
    }

    /// Thread-safe version of refresh_local_index for use in async refresh
    async fn refresh_local_index_thread(
        config_folder: PathBuf,
        schema_folder: PathBuf,
        local_maps: Arc<Mutex<HashSet<LocalAvailableMap>>>,
    ) {
        let new_local_maps = Self::scan_map_folder(&config_folder, &schema_folder);

        let mut maps = local_maps.lock().await;
        *maps = new_local_maps;
    }

    /// Thread-safe version of refresh_schema_index for use in async refresh
    async fn refresh_schema_index_thread(
        schema_folder: PathBuf,
        local_schemas: Arc<Mutex<HashSet<LocalAvailableSchema>>>,
    ) {
        let found = Self::scan_schema_folder(&schema_folder);

        let mut schemas = local_schemas.lock().await;
        *schemas = found;
    }

    /// Thread-safe version of refresh_remote_index for use in async refresh
    async fn refresh_remote_index_thread(
        base_url: String,
        remote_maps: Arc<Mutex<Option<Vec<NetworkAvailableMap>>>>,
    ) {
        match fetch_text(&(base_url.clone() + "catalog.json")).await {
            Ok(res) => {
                match serde_json::from_str::<Vec<NetworkAvailableMap>>(&res) {
                Ok(updated_index) => {
                    let mut maps = remote_maps.lock().await;
                    *maps = Some(updated_index);
                }
                Err(err) => {
                    log::error!("Unable to parse returned response: {}\n{}", err, &res);
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
    pub async fn load_map(
        &mut self,
        author: String,
        name: String,
        version: u32,
    ) -> Result<GameMap, ApiRetrievalError> {
        // Look for local maps
        let should_refresh = {
            let local_maps = self.local_maps.lock().await;
            for local in local_maps.iter() {
                if name == local.name && author == local.author
                    && local.version == version {
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
            false
        }; // Lock is dropped here

        if should_refresh {
            self.refresh_local_index().await;
            // Try once more after refresh
            let local_maps = self.local_maps.lock().await;
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

        // try to retrieve remote
        let remote_maps_guard = self.remote_maps.lock().await;
    if let Some(ref remote_maps) = *remote_maps_guard {
        for remote in remote_maps.iter() {
            if name == remote.name && author == remote.author {
                let request_url = self.base_url.clone() + &remote.url;
                if let Ok(content) = fetch_text(&request_url).await {
                    if let Ok(map) = serde_json::from_str::<GameMap>(&content) {
                        // Cache to disk
                        let cache_dir = PathBuf::from(&self.config_folder);
                        if let Err(e) = fs::create_dir_all(&cache_dir) {
                            log::warn!("Failed to create cache dir: {}", e);
                        } else {
                            let filename = format!("{}_{}_{}.json", author, name, version);
                            let cache_path = cache_dir.join(&filename);
                            match serde_json::to_string_pretty(&map) {
                                Ok(json) => {
                                    if let Err(e) = fs::write(&cache_path, &json) {
                                        log::warn!("Failed to write cached map: {}", e);
                                    } else {
                                        log::debug!("Cached map to {:?}", cache_path);
                                        // Update local index with the new entry
                                        let mut local = self.local_maps.lock().await;
                                        local.insert(LocalAvailableMap {
                                            author: remote.author.clone(),
                                            name: remote.name.clone(),
                                            version: remote.version,
                                            path: cache_path,
                                        });
                                    }
                                }
                                Err(e) => {
                                    log::warn!("Failed to serialize map for caching: {}", e);
                                }
                            }
                        }
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

        Err(ApiRetrievalError::MapNotFound(format!(
            "No Map found for:  Author:{}, name:{}, version:{}",
            author, name, version
        )))
    }

    /// Loads the camera schema with the given ID.
    ///
    /// The index holds one entry per file. Two files may share an ID. The
    /// loader takes the highest version. A file that fails to parse falls back
    /// to the next lower version.
    ///
    /// The remote source does not exist yet. The search stays local.
    pub async fn load_cam(&self, id: &str) -> Result<CamConfig, ApiRetrievalError> {
        Self::load_schema_from(&self.local_cams, id).await
    }

    /// Loads a camera schema straight out of the index.
    ///
    /// The caller holds the index, not the whole manager. The VRC loop uses
    /// this form so that a schema read never blocks the map loader.
    pub async fn load_schema_from(
        index: &Arc<Mutex<HashSet<LocalAvailableSchema>>>,
        id: &str,
    ) -> Result<CamConfig, ApiRetrievalError> {
        let mut candidates: Vec<LocalAvailableSchema> = {
            let schemas = index.lock().await;
            schemas
                .iter()
                .filter(|entry| entry.id == id)
                .cloned()
                .collect()
        };

        if candidates.is_empty() {
            return Err(ApiRetrievalError::MapNotFound(format!(
                "No camera schema found for id: {}",
                id
            )));
        }

        // Highest version first.
        candidates.sort_unstable_by(|a, b| b.version.cmp(&a.version));

        let mut last_error = String::new();
        for entry in &candidates {
            match cam_schema::read_schema(&entry.path) {
                Ok(parsed) => match parsed.validate() {
                    Ok(()) => return Ok(parsed),
                    Err(err) => {
                        last_error = format!("{:?}: {}", entry.path, err);
                        log::warn!("Rejected camera schema {}", last_error);
                    }
                },
                Err(err) => {
                    last_error = format!("{:?}: {}", entry.path, err);
                    log::warn!("Unable to read camera schema {}", last_error);
                }
            }
        }

        Err(ApiRetrievalError::BadResponseFromServer(format!(
            "No usable camera schema for id {}. Last error: {}",
            id, last_error
        )))
    }

    /// Re-indexes the local config files.
    /// Each config file is "probably" valid, it atleast has each of the needed fields.
    pub async fn refresh_local_index(&mut self) {
        // Find already locally cached maps.
        let new_local_maps = Self::scan_map_folder(&self.config_folder, &self.cam_folder);

        let mut maps = self.local_maps.lock().await;
        *maps = new_local_maps;
    }

    /// Re-indexes the local camera schema files.
    ///
    /// The scan reads every file under the schema folder. A file that does not
    /// hold a schema drops out of the index with a warning.
    pub async fn refresh_schema_index(&self) {
        let found = Self::scan_schema_folder(&self.cam_folder);

        let mut schemas = self.local_cams.lock().await;
        *schemas = found;
    }

    /// Walks the config folder and reads every map file.
    ///
    /// The schema folder often sits under the config folder. The walk skips it,
    /// because a camera schema is not a map.
    fn scan_map_folder(folder: &PathBuf, skip: &PathBuf) -> HashSet<LocalAvailableMap> {
        let mut new_local_maps = HashSet::new();

        for entry in WalkDir::new(folder).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            if entry.path().starts_with(skip) {
                continue;
            }

            match fs::read_to_string(entry.path()) {
                Ok(content) => match serde_json::from_str::<GameMap>(&content) {
                    Ok(game_map) => {
                        let map = LocalAvailableMap {
                            author: game_map.identification.author_name,
                            name: game_map.identification.map_name,
                            version: game_map.identification.map_version,
                            path: entry.clone().into_path(),
                        };

                        if !new_local_maps.insert(map) {
                            log::warn!(
                                "Duplicate config files, Will be ignored: {:?}",
                                entry.file_name()
                            );
                        }
                    }
                    // Log WHY it failed instead of discarding the error.
                    Err(e) => log::warn!("Unable to parse {:?} as GameMap: {}", entry.path(), e),
                },
                Err(e) => log::warn!("Unable to read file {:?}: {}", entry.path(), e),
            }
        }

        new_local_maps
    }

    /// Walks the schema folder and reads every schema file.
    ///
    /// The reader takes plain JSON and markdown with a fenced JSON block.
    fn scan_schema_folder(folder: &PathBuf) -> HashSet<LocalAvailableSchema> {
        let mut found = HashSet::new();

        // The user drops schema files in here, so the folder must exist.
        if !folder.is_dir() {
            if let Err(err) = fs::create_dir_all(folder) {
                log::warn!("Unable to create the schema folder {:?}: {}", folder, err);
                return found;
            }
        }

        for entry in WalkDir::new(folder).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }

            match cam_schema::read_schema(entry.path()) {
                Ok(parsed) => {
                    if let Err(err) = parsed.validate() {
                        log::warn!("Unusable camera schema {:?}: {}", entry.path(), err);
                        continue;
                    }

                    let record = LocalAvailableSchema {
                        id: parsed.id,
                        version: parsed.version,
                        path: entry.clone().into_path(),
                    };

                    if !found.insert(record) {
                        log::warn!(
                            "Duplicate camera schema, Will be ignored: {:?}",
                            entry.file_name()
                        );
                    }
                }
                Err(err) => log::warn!(
                    "Unable to parse {:?} as a camera schema: {}",
                    entry.path(),
                    err
                ),
            }
        }

        found
    }

    /// Calls to refresh files available on the remote index.
    /// Fills self.available_maps with result.
    pub async fn refresh_remote_index(&mut self) {
        match fetch_text(&(self.base_url.clone() + "catalog.json")).await {
            Ok(text) => match serde_json::from_str::<Vec<NetworkAvailableMap>>(&text) {
                Ok(updated_index) => {
                    let mut maps = self.remote_maps.lock().await;
                    *maps = Some(updated_index);
                }
                Err(err) => {
                    log::error!("Unable to parse returned response: {}\n{}", err, &text);
                }
            },
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
