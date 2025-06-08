use super::{haptic_node::HapticNode, input_node::InputNode};

pub trait Interpolate {
    fn interp(&self, node: &Vec<HapticNode>, in_nodes: Vec<&InputNode>) -> Vec<f32>;
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(tag = "algo", content = "state")]
/// Interpolation Algorithm Options.
///
/// Each entry implements the mapping::Interpolate trait and provides a self-contained
/// method for interpolation
pub enum InterpAlgo {
    Gaussian(GaussianState),
}

impl Interpolate for InterpAlgo {
    /// node: the haptic nodes (output positions) that will be used to determine the feedback value
    ///
    /// in_nodes: The haptic Nodes that will be used to calculate the output value
    fn interp(&self, node: &Vec<HapticNode>, in_nodes: Vec<&InputNode>) -> Vec<f32> {
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
    merge: f32, // TODO: Maybe look at caching interacted values?
    sigma_const: f32,
}

impl GaussianState {
    /// Creates a new Gaussian interpolation instance
    /// Initializes parameters
    pub fn new(merge: f32, falloff: f32, cutoff: f32) -> GaussianState {
        let mut g = GaussianState {
            sigma: 0.0,
            cutoff: cutoff,
            merge: merge,
            sigma_const: 100.,
        };
        g.set_fallof(falloff);
        return g;
    }

    /// Sets the devices falloff in meters to reach 5% of input value.
    pub fn set_fallof(&mut self, falloff: f32) {
        self.sigma = falloff / (-2.0 * 0.05_f32.ln());
        self.sigma_const = 2.0 * self.sigma.powi(2);
    }

    /// used in interp function
    fn gaussian_kernel(&self, distance: f32) -> f32 {
        (-distance.powi(2) / self.sigma_const).exp()
    }

    /// returns the straight interpolation for the node.
    fn single_node(&self, node: &HapticNode, in_nodes: &Vec<&InputNode>) -> f32 {
        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for in_node in in_nodes.iter() {
            // if the game node should influence the device node
            if node.interacts(&in_node.haptic_node) {
                let distance = node.dist(&in_node.haptic_node);
                // if below our threshold, return early with that nodes intensity
                if !distance.is_nan() && distance < self.cutoff {
                    let weight = self.gaussian_kernel(distance);
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
                    node.x,
                    node.y,
                    node.z
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

impl Interpolate for GaussianState {
    /// Takes in the list of output nodes on a device, and the input nodes that should influence it.
    fn interp(&self, node_list: &Vec<HapticNode>, in_nodes: Vec<&InputNode>) -> Vec<f32> {
        // For each output node, evaluate the Gaussian kernel against the full set of inputs.
        node_list
            .iter()
            .map(|out_node| self.single_node(out_node, &in_nodes))
            .collect()
    }
}
