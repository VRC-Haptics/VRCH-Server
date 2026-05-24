pub mod event_map;

use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use directories::ProjectDirs;
use event_map::{BaseMessage, GameMapping};

use crate::network::{self, fetch_text};
use crate::file::{resolve_dir, Directory};
use crate::log_err;

const CACHE_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

fn cache_path(app_id: &str) -> PathBuf {
    let folder = resolve_dir(Directory::BhapticsCache);
    let mut file = folder.join(format!("bhaptics_cache_{}", app_id));
    file.add_extension("json");
    file
}

fn read_cache(path: &PathBuf) -> Option<GameMapping> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    if SystemTime::now().duration_since(modified).ok()? > CACHE_MAX_AGE {
        return None;
    }
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str::<GameMapping>(&data).ok()
}

fn write_cache(path: &PathBuf, mapping: &GameMapping) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(mapping)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, json)?;
    Ok(())
}

/// Tries to get a response from the api and parse it.
/// Results are cached to disk and reused for up to 24 hours.
///
/// Set version to -1 to get latest version.
pub async fn fetch_mappings(
    api_key: String,
    app_id: String,
    version: i32,
) -> Result<GameMapping, FetchMappingsError> {
    let path = cache_path(&app_id);

    if let Some(cached) = read_cache(&path) {
        log::info!("Using cached bHaptics mappings for {}", app_id);
        return Ok(cached);
    }

    let url = format!(
        "http://sdk-apis.bhaptics.com/api/v1/haptic-definitions/workspace-v3/latest?latest-version={}&api-key={}&app-id={}",
        version, api_key, app_id
    );

    let resp = fetch_text(&url).await.map_err(FetchMappingsError::HttpError)?;
    let body = resp;
    let msg: BaseMessage =
        serde_json::from_str(&body).map_err(|e| FetchMappingsError::DeserializeError(e, body))?;


    log_err!(write_cache(&path, &msg.message));
    

    Ok(msg.message)
}

#[derive(Debug)]
pub enum FetchMappingsError {
    HttpError(network::HttpError),
    DeserializeError(serde_json::Error, String),
}