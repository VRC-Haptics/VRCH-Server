use serde::de::{self, Deserializer, IntoDeserializer};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json;

/// Interprets a quote escaped json ojbect as raw object
/// E.G.: `"{"contents": "here"}"` will be treated as: `{"contents": "here"}`
pub fn skip_outer_quotes<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where 
    D: Deserializer<'de>,
    T: DeserializeOwned + std::fmt::Debug,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    if let serde_json::Value::String(s) = value {
        // s is an owned String; T must be fully owned as well.
        let out = serde_json::from_str(&s).map_err(de::Error::custom);
        
        if let Ok(success) = out {
            return Ok(success);
        } else {
            log::error!("Failed on: {}", s);
            return out;
        }

    } else {
        // For non-string values, deserialize directly.
        T::deserialize(value.into_deserializer()).map_err(de::Error::custom)
    }
}
