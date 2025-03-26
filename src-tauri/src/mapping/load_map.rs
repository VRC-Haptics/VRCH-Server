use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct MapData {
    pub device_map: Vec<Device>,
    pub game_map: Vec<GameMapNode>,
    pub meta: Meta,
}