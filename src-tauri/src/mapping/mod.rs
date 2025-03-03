pub mod haptic_node;

use haptic_node::HapticNode;

/// Descriptors for location groups.
/// Allows for segmented Interpolation
#[derive(PartialEq, serde::Deserialize, serde::Serialize)]
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

/// Maps the game nodes to this devices nodes
pub struct HapticMap {
    pub game_map: Option<Vec<HapticNode>>,
    pub device_map: Option<Vec<HapticNode>>,
    /// Values between 0 and 1 representing the game nodes power keyed to game_map indices
    pub game_intensity: Vec<f32>,
    /// Last values returned to get()
    pub last_sent: Vec<f32>,
    /// At what radius the games nodes influence is 5%. Only propogates changes if set_fallof() is called
    pub falloff_distance: f32,
    /// constant used in gaussian kernel (units:meters)
    sigma: f32,
}

impl HapticMap {
    /// Creates empty HapticMap
    pub fn new(falloff_distance: f32) -> HapticMap {
        let gaussian_sigma = falloff_distance / (-2.0 * 0.05_f32.ln());
        return HapticMap {
            game_map: None,
            device_map: None,
            game_intensity: Vec::new(),
            last_sent: Vec::new(),
            falloff_distance: falloff_distance,
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
    /// Returns None if values haven't changed since last get() call.
    pub fn get(&mut self) -> Result<Option<&Vec<f32>>, HapticMapError> {
        let mut new_values = self.last_sent.clone();

        // if we have both a game and device map
        if let Some(game_map) = &self.device_map {
            if let Some(device_map) = &self.device_map {
                // get the interpolated position for each point
                for (index, device_node) in device_map.iter().enumerate() {
                    new_values[index] =
                        interp_maps(device_node, game_map, &self.game_intensity, self.sigma, self.falloff_distance);
                }

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

    pub fn set(&mut self, intensity_values: &Vec<f32>) -> Result<(), HapticMapError> {
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
fn interp_maps(
    device_node: &HapticNode,
    game_nodes: &Vec<HapticNode>,
    game_intensity: &Vec<f32>,
    sigma: f32,
    cutoff: f32,
) -> f32 {
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for (index, other_node) in game_nodes.iter().enumerate() {
        // if the game node should influence the device node
        if device_node.interacts(other_node) {
            let distance = device_node.dist(other_node);
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
