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
    merge: f32, // TODO: Maybe look at caching interacted values?
    at_edge: f32,
}

impl GaussianState {
    /// Creates a new Gaussian interpolation instance
    /// Initializes parameters
    pub fn new(merge: f32, at_edge: f32) -> GaussianState {
        GaussianState {
            merge: merge,
            at_edge: at_edge,
        }
    }

    /// used in interp function to get a weight between zero and one for the distance between two points.
    #[inline]
    fn gaussian_kernel(&self, distance: f32, max_radius: f32) -> f32 {
        debug_assert!(distance >= 0.0 && max_radius > 0.0 && self.at_edge > 0.0 && self.at_edge < 1.0);

        let sigma = max_radius / (-2.0 * self.at_edge.ln()).sqrt();

        (-0.5 * (distance / sigma).powi(2)).exp()
    }

    /// returns the straight interpolation for the node.
    fn single_node(&self, node: &HapticNode, in_nodes: &Vec<&InputNode>) -> f32 {
        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for in_node in in_nodes.iter() {
            // if the game node should influence the device node
            if node.interacts(&in_node.haptic_node) {
                let distance = node.dist(&in_node.haptic_node);
                let max_radius = in_node.get_radius();
                // if below our threshold, add influence
                if !distance.is_nan() && distance < max_radius {
                    let weight = self.gaussian_kernel(distance, max_radius);
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
        let mut out_list = vec![0.0; node_list.len()];
        for (index, node) in node_list.iter().enumerate() {
            out_list[index] = self.single_node(node, &in_nodes);
        }

        out_list
    }
}
