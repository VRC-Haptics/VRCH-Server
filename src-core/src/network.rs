use std::fmt;

#[derive(Debug)]
pub struct HttpError(String);

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for HttpError {}

#[cfg(not(feature = "tauri-get"))]
pub async fn fetch_text(url: &str) -> Result<String, HttpError> {
    use reqwest;

    let bytes = reqwest::get(url)
        .await
        .map_err(|e| HttpError(e.to_string()))?
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| HttpError(e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| HttpError(e.to_string()))
}

#[cfg(feature = "tauri-get")]
pub async fn fetch_text(url: &str) -> Result<String, HttpError> {
    use tauri_plugin_http::reqwest;

    let bytes = reqwest::get(url)
        .await
        .map_err(|e| HttpError(e.to_string()))?
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| HttpError(e.to_string()))?;
    String::from_utf8(bytes).map_err(|e| HttpError(e.to_string()))
}