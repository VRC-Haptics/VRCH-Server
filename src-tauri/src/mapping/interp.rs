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
        let mut out_list: Vec<f32> = vec![0.0; node_list.len()];

        // calimed inputs share an index for the referenced claimed value
        // (node, index in respective input list)
        let mut claimed_inputs: Vec<(&InputNode, usize)> = vec![];
        let mut claimed_outputs: Vec<(&HapticNode, usize)> = vec![];

        // gather all perfect pairing's first.
        for (out_index, node) in node_list.iter().enumerate() {
            for (in_index, input) in in_nodes.iter().enumerate() {
                let distance = node.dist(&input.haptic_node);
                // if below our threshold, return early it should be claimed
                if distance <= self.merge {
                    claimed_inputs.push((*input, in_index));
                    claimed_outputs.push((node, out_index));
                    break;
                }
            }
        }

        //move perfect pairings
        for ((input, _in_idx), (_out_node, out_idx)) in
            claimed_inputs.iter().zip(claimed_outputs.iter())
        {
            out_list[*out_idx] = input.get_intensity()
        }

        // need to setup these...
        // all nodes that haven't been claimed
        let unique_inputs: Vec<&InputNode> = in_nodes
            .iter()
            .enumerate()
            .filter_map(|(i, input)| {
                if !claimed_inputs.iter().any(|&(_, ci)| ci == i) {
                    Some(*input)
                } else {
                    None
                }
            })
            .collect();

        let unique_outputs: Vec<(&HapticNode, usize)> = node_list
            .iter()
            .enumerate()
            .filter_map(|(i, node)| {
                if !claimed_outputs.iter().any(|&(_, co)| co == i) {
                    Some((node, i))
                } else {
                    None
                }
            })
            .collect();

        // fill in the rest of the out_list (all indices should be convered at some point.)
        for (output, main_index) in unique_outputs.iter() {
            out_list[*main_index] = self.single_node(output, &unique_inputs);
        }

        return out_list;
    }
}
