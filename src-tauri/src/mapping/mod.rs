pub mod haptic_node;

use haptic_node::HapticNode;

/// Descriptors for location groups.
/// Allows for segmented Interpolation
#[derive(PartialEq, serde::Deserialize, serde::Serialize, Clone, Debug)]
pub enum NodeGroup {
    Head,
    ArmRight,
    ArmLeft,
    TorsoRight,
    TorsoLeft,
    TorsoFront,
    TorsoBack,
    LegRight,
    LegLeft,
    FootRight,
    FootLeft,
}

impl NodeGroup {
    /// Given a string containing at least 2 raw bytes, interpret the first two bytes as
    /// a little-endian u16 bitflag and convert that into a Vec<NodeGroup>.
    pub fn parse_from_str(s: &str) -> Vec<NodeGroup> {
        let bytes = s.as_bytes();
        if bytes.len() < 2 {
            // Not enough data; return an empty vector
            return Vec::new();
        }
        let flag = u16::from_le_bytes([bytes[0], bytes[1]]);
        Self::from_bitflag(flag)
    }

    /// Converts a slice of NodeGroup into a bitflag.
    pub fn to_bitflag(groups: &[NodeGroup]) -> u16 {
        let mut flag: u16 = 0;
        for group in groups {
            flag |= match group {
                NodeGroup::Head => 1 << 0,
                NodeGroup::ArmRight => 1 << 1,
                NodeGroup::ArmLeft => 1 << 2,
                NodeGroup::TorsoRight => 1 << 3,
                NodeGroup::TorsoLeft => 1 << 4,
                NodeGroup::TorsoFront => 1 << 5,
                NodeGroup::TorsoBack => 1 << 6,
                NodeGroup::LegRight => 1 << 7,
                NodeGroup::LegLeft => 1 << 8,
                NodeGroup::FootRight => 1 << 9,
                NodeGroup::FootLeft => 1 << 10,
            }
        }
        flag
    }

    /// Converts a bitflag back into a vector of NodeGroup variants.
    pub fn from_bitflag(flag: u16) -> Vec<NodeGroup> {
        let mut groups = Vec::new();
        if flag & (1 << 0) != 0 { groups.push(NodeGroup::Head); }
        if flag & (1 << 1) != 0 { groups.push(NodeGroup::ArmRight); }
        if flag & (1 << 2) != 0 { groups.push(NodeGroup::ArmLeft); }
        if flag & (1 << 3) != 0 { groups.push(NodeGroup::TorsoRight); }
        if flag & (1 << 4) != 0 { groups.push(NodeGroup::TorsoLeft); }
        if flag & (1 << 5) != 0 { groups.push(NodeGroup::TorsoFront); }
        if flag & (1 << 6) != 0 { groups.push(NodeGroup::TorsoBack); }
        if flag & (1 << 7) != 0 { groups.push(NodeGroup::LegRight); }
        if flag & (1 << 8) != 0 { groups.push(NodeGroup::LegLeft); }
        if flag & (1 << 9) != 0 { groups.push(NodeGroup::FootRight); }
        if flag & (1 << 10) != 0 { groups.push(NodeGroup::FootLeft); }
        groups
    }
}

/// Maps the game nodes to this devices nodes
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct HapticMap {
    pub game_map: Option<Vec<HapticNode>>,
    pub device_map: Option<Vec<HapticNode>>,
    /// Values between 0 and 1 representing the game nodes power keyed to game_map indices
    pub game_intensity: Vec<f32>,
    /// Last values returned to get()
    pub last_sent: Vec<f32>,
    /// At what radius the games nodes influence is 5%. Only propogates changes if set_fallof() is called
    pub falloff_distance: f32,
    /// At what radius in meters to merge two nodes
    pub merge_distance: f32,
    /// constant used in gaussian kernel (units:meters)
    sigma: f32,
}

impl HapticMap {
    /// Creates empty HapticMap
    pub fn new(falloff_distance: f32, merge_distance:f32) -> HapticMap {
        let gaussian_sigma = falloff_distance / (-2.0 * 0.05_f32.ln());
        return HapticMap {
            game_map: None,
            device_map: None,
            game_intensity: Vec::new(),
            last_sent: Vec::new(),
            falloff_distance: falloff_distance,
            merge_distance: merge_distance,
            sigma: gaussian_sigma,
        };
    }

    /// Fills this HapticMap with these nodes and performs calculations.
    pub fn set_device_map(&mut self, device_map: Vec<HapticNode>) {
        self.device_map = Some(device_map);
    }

    /// Sets the game node map.
    pub fn set_game_map(&mut self, game_map: Vec<HapticNode>) {
        self.game_map = Some(game_map);
    }

    /// Sets the devices fallof in meters to reach 5% of input value.
    pub fn set_fallof(&mut self, falloff: f32) {
        self.sigma = falloff / (-2.0 * 0.05_f32.ln());
    }

    /// Returns array of power percentage.
    /// Reutrns Error if device map not set and game map not set.
    /// Returns None if values haven't changed since last get() call.
    pub fn get_device_nodes(&mut self) -> Result<Option<&Vec<f32>>, HapticMapError> {
        // Check that both game_map and device_map are set
        if let Some(game_map) = &self.game_map {
            if let Some(device_map) = &self.device_map {
                // Initialize new_values with the correct length.
                let mut new_values = if self.last_sent.len() == device_map.len() {
                    self.last_sent.clone()
                } else {
                    vec![0.0; device_map.len()]
                };
    
                // Iterate over the device_map and update new_values accordingly.
                for (index, device_node) in device_map.iter().enumerate() {
                    new_values[index] = interp_maps(
                        device_node,
                        game_map,
                        &self.game_intensity,
                        self.sigma,
                        self.falloff_distance,
                        self.merge_distance,
                    );
                }
    
                // If the values have changed, update last_sent and return them.
                if new_values != self.last_sent {
                    self.last_sent = new_values;
                    return Ok(Some(&self.last_sent));
                } else {
                    return Ok(None);
                }
            } else {
                return Err(HapticMapError::DeviceMapNotSet);
            }
        } else {
            return Err(HapticMapError::GameMapNotSet);
        }
    }
    

    /// Called when the game needs to update it's commanded intensity values.
    pub fn set_game_nodes(&mut self, intensity_values: &Vec<f32>) -> Result<(), HapticMapError> {
        if let Some(game_map) = &self.game_map {
            if game_map.len() == intensity_values.len() {
                self.game_intensity = intensity_values.to_vec();
                return Ok(());
            } else {
                return Err(HapticMapError::LengthsNotEqual);
            }
        } else {
            return Err(HapticMapError::GameMapNotSet);
        }
    }

    pub fn set_index(&mut self, index: usize, intensity: f32) -> Result<(), HapticMapError> {
        if let Some(_) = &self.game_map {
            self.game_intensity[index] = intensity;
            return Ok(());
            
        } else {
            return Err(HapticMapError::GameMapNotSet);
        }
    }
}

pub enum HapticMapError {
    /// Lengths of the input array and game_map aren't equal
    LengthsNotEqual,
    /// This device's game map isn't set yet
    GameMapNotSet,
    /// This device's device map isn't set yet
    DeviceMapNotSet,
}

/// Get feedback strength interpolated from game values.
pub fn interp_maps(
    device_node: &HapticNode,
    game_nodes: &Vec<HapticNode>,
    game_intensity: &Vec<f32>,
    sigma: f32,
    cutoff: f32,
    merge: f32,
) -> f32 {
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for (index, other_node) in game_nodes.iter().enumerate() {
        // if the game node should influence the device node
        if device_node.interacts(other_node) {
            let distance = device_node.dist(other_node);
            // if below our threshold, return early with that nodes intensity
            if distance <= merge {
                return game_intensity[index];
            }
            if !distance.is_nan() && distance > cutoff {
                let weight = gaussian_kernel(distance, sigma);
                numerator += weight * game_intensity[index];
                denominator += weight;
            }
        }
    }

    if denominator > 0.0 {
        let result = numerator / denominator;
        if result > 1.0 {
            println!(
                "Strength greater than one on device node: x:{} y:{} z:{}",
                device_node.x, device_node.y, device_node.z
            );
            return 1.0;
        } else {
            return result;
        }
    } else {
        return 0.0;
    }
}

fn gaussian_kernel(distance: f32, sigma: f32) -> f32 {
    (-distance.powi(2) / (2.0 * sigma.powi(2))).exp()
}
