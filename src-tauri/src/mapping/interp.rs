use super::{
    input_node::InputNode, 
    haptic_node::HapticNode
};

pub trait Interpolate {
    fn interp(&self, node: &HapticNode, in_nodes: Vec<&InputNode>) -> f32;
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(tag = "algo", content = "state")]
/// Interpolation Algorithm Options.
/// 
/// Each entry implements the mapping::Interpolate trait and provides a self-contained 
/// method for interpolation
pub enum InterpAlgo {
    Gaussian(GaussianState)
}

impl Interpolate for InterpAlgo {
    /// node: the haptic node (output position) that will be used to determine the feedback value
    /// 
    /// in_nodes: The haptic Nodes that will be used to calculate the output value
    fn interp(&self, node: &HapticNode, in_nodes: Vec<&InputNode>) -> f32 {
        match self {
            InterpAlgo::Gaussian(state) => state.interp(node, in_nodes),
            // add other algo's here
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
/// A container class holding variables required for gaussian distribution calculations
/// 
/// Provides `Interpolate` Implementation  
pub struct GaussianState {
    sigma: f32,
    cutoff: f32,
    merge: f32
}

impl GaussianState {
    /// Creates a new Gaussian interpolation instance
    /// Initializes parameters
    pub fn new(merge: f32, falloff: f32, cutoff: f32) -> GaussianState {
        let mut  g = GaussianState {
            sigma: 0.0,
            cutoff: cutoff,
            merge: merge,
        };
        g.set_fallof(falloff);
        return g;
    }

    /// Sets the devices fallof in meters to reach 5% of input value.
    pub fn set_fallof(&mut self, falloff: f32) {
        self.sigma = falloff / (-2.0 * 0.05_f32.ln());
    }

    fn gaussian_kernel(&self, distance: f32, sigma: f32) -> f32 {
        (-distance.powi(2) / (2.0 * sigma.powi(2))).exp()
    }
}

impl Interpolate for GaussianState {
    fn interp(&self, node: &HapticNode, in_nodes: Vec<&InputNode>) -> f32 {
        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for in_node in in_nodes.iter() {
            // if the game node should influence the device node
            if node.interacts(&in_node.haptic_node) {
                let distance = node.dist(&in_node.haptic_node);
                // if below our threshold, return early with that nodes intensity
                if distance <= self.merge {
                    return in_node.get_intensity();
                }
                if !distance.is_nan() && distance < self.cutoff {
                    let weight = self.gaussian_kernel(distance, self.sigma);
                    numerator += weight * in_node.get_intensity();
                    denominator += weight;
                }
            }
        }

        if denominator > 0.0 {
            let result = numerator / denominator;
            if result > 1.0 {
                log::error!(
                    "Strength greater than one on device node: x:{} y:{} z:{}",
                    node.x, node.y, node.z
                );
                return 1.0;
            } else {
                return result;
            }
        } else {
            return 0.0;
        }
    }
}