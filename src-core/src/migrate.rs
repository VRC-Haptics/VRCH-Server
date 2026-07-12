use serde_json::Value;

use crate::state::{self, Config};


pub enum Versions {
    V0,
    V1,
}

impl Versions {
    pub fn from_num(num: u32) -> Option<Self> {
        match num {
            0 => Some(Self::V0),
            _ => None
        }
    }
}

pub fn migrate(value: Value) -> Option<Config> {
    let num = value
        .get("version")
        .and_then(serde_json::Value::as_u64)
        .map(|v| v as u32).unwrap_or(0);
    let version = Versions::from_num(num)?;

    match version {
        Versions::V0 => v0_to_v1(value),
        Versions::V1 => serde_json::from_value::<Config>(value).ok()
    }
}

pub fn v0_to_v1(value: Value) -> Option<Config> {
    // add version tag.
    // interpret as v1.
    // migrate default scaling per known device.
    None
}
