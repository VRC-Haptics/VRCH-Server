pub mod event_map;

use event_map::{BaseMessage, GameMapping};
use tauri::http::request;
use tauri_plugin_http::reqwest::blocking::get;

/// Tries to get a response from the api and parse it.
/// 
/// set version to -1 to get latest version.
pub fn fetch_mappings(api_key: String, app_id: String, version: i32) -> Result<GameMapping, FetchMappingsError> {
    let url = format!("http://sdk-apis.bhaptics.com/api/v1/haptic-definitions/workspace-v3/latest?latest-version={}&api-key={}&app-id={}", version, api_key, app_id);
    match get(url) {
        Ok(resp) => {
            //log::trace!("{:?}", resp);
            match resp.text() {
                Ok(body) => {
                    match serde_json::from_str::<BaseMessage>(&body) {
                        Ok(msg) => Ok(msg.),
                        Err(err) => Err(FetchMappingsError::DeserializeError(err, body)),
                    }
                }
                Err(err) => {return Err(FetchMappingsError::HttpError(err))}
            }
        }
        Err(err) => {
            return Err(FetchMappingsError::HttpError(err))
        }
    }
}

#[derive(Debug)]
pub enum FetchMappingsError {
    HttpError(reqwest::Error),
    DeserializeError(serde_json::Error, String)
}
