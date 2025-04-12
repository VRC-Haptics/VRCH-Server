use crate::mapping::haptic_node::HapticNode;

use std::fs;
use std::io;
use std::path::PathBuf;
use walkdir::WalkDir;

use super::OscPath;

/// Searches for a file named "<author>_<name>_<version>.json" within the provided
/// directories (and their subdirectories) and returns the first instance found.
///
/// # Arguments
///
/// * `author` - The author name used in the file name.
/// * `name` - The name used in the file name.
/// * `version` - The version number used in the file name.
/// * `paths` - A vector of directories to search.
///
/// # Returns
///
/// * `Ok(GameMap)` if the file is found and successfully parsed.
/// * `Err(io::Error)` if the file is not found or if there is a parsing/IO error.
pub fn load_vrc_config(
    author: String,
    name: String,
    version: u32,
    paths: Vec<PathBuf>,
) -> Result<GameMap, io::Error> {
    // Construct the expected file name
    let file_name = format!("{}_{}_{}.json", author, name, version);

    // Iterate over each directory provided
    for dir in paths {
        // Walk the directory recursively
        for entry in WalkDir::new(&dir).into_iter().filter_map(Result::ok) {
            // Check if the current entry is a file and if its name matches
            if entry.file_type().is_file() &&
               entry.file_name().to_string_lossy() == file_name
            {
                // Read the file to a string
                let content = fs::read_to_string(entry.path())?;
                // Deserialize JSON content into GameMap
                let game_map: GameMap = serde_json::from_str(&content)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                return Ok(game_map);
            }
        }
    }
    // Return an error if no file was found
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("File {} not found in provided paths", file_name),
    ))
}

/// Filled with values from a config json file.
/// Provides all information needed to fully define the avatar prefab.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct GameMap {
    pub nodes: Vec<ConfNode>,
    pub meta: ConfMetadata,
}

/// Haptic Node information from the game config
/// Contains more information than the default HapticNode to help with locating 
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct ConfNode {
    #[serde(rename = "nodeData")]
    pub node_data: HapticNode,
    pub address: String,
    pub radius: f32,
    #[serde(rename = "targetBone")]
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

impl ToString for TargetBone {
    fn to_string(&self) -> String {
        self.to_str().to_string()
    }
}

/// Metadata from the json config
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct ConfMetadata {
    pub map_name: String,
    pub map_version: u32,
    pub map_author: String,
    pub menu: StandardMenuParameters,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct StandardMenuParameters {
    pub intensity: OscPath,
}