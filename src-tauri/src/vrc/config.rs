use crate::mapping::haptic_node::HapticNode;

/// Filled with values from a config json file.
/// Provides locations values for 
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct GameMap {
    pub nodes: Vec<confNode>,
    pub meta: confMetadata,
}

/// Haptic Node information from the game config
/// Contains more information than the default HapticNode to help with locating 
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct confNode {
    pub node_data: HapticNode,
    pub address: String,
    pub radius: f32,
    pub target_bone: TargetBone,
}

/// The bone that the node is parented to in the prefab.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub enum TargetBone {
    Head,

}

impl TargetBone {
    pub fn to_str(&self) -> &str {
        match self {
            TargetBone::Head => "Head",
        }
    }
}

/// Metadata from the json config
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct confMetadata {
    pub map_name: String,
    pub map_version: u32,
    pub map_author: String,
}