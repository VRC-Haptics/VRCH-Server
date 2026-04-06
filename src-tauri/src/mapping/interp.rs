use enum_dispatch::enum_dispatch;

use crate::{mapping::input_node::InputType, state::PerDevice};

use super::{haptic_node::HapticNode, input_node::InputNode};

#[enum_dispatch(InterpAlgo)]
pub trait Interpolate {
    /// Implementations need to take into account that all lengths could be different.
    fn interp(&self, haptic_nodes: &[HapticNode], output: &mut[f32], in_nodes: &[InputNode], settings: &PerDevice);
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(tag = "algo", content = "state")]
/// Interpolation Algorithm Options.
///
/// Each entry implements the mapping::Interpolate trait and provides a self-contained
/// method for interpolation
#[enum_dispatch]
pub enum InterpAlgo {
    /// Uses a gaussian distribution on the array of input nodes an weights them to determine output.
    Gaussian(GaussianState),
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
/// A container class holding variables required for gaussian distribution interpolations.
pub struct GaussianState {
    merge: f32, // TODO: Maybe look at caching interacted values?
    at_edge: f32,
}

impl Default for GaussianState {
    fn default() -> Self {
        GaussianState { merge: 0.002, at_edge: 0.05}
    }
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
        debug_assert!(
            distance >= 0.0 && max_radius > 0.0 && self.at_edge > 0.0 && self.at_edge < 1.0
        );

        let sigma = max_radius / (-2.0 * self.at_edge.ln()).sqrt();

        (-0.5 * (distance / sigma).powi(2)).exp()
    }

    /// returns the straight interpolation for the node.
    fn single_node(&self, node: &HapticNode, in_nodes: &[InputNode]) -> f32 {
        let mut interp_numerator = 0.0;
        let mut interp_denominator = 0.0;
        let mut add_numerator = 0.0;
        let mut add_denominator = 0.0;

        for in_node in in_nodes.iter() {
            // if the game node should influence the device node
            if node.interacts(&in_node.haptic_node) {
                let distance = node.dist(&in_node.haptic_node);
                // if below our threshold, add influence
                if !distance.is_nan() && distance < in_node.get_radius() {
                    // handle different interpolation layers
                    match in_node.input_type {
                        InputType::INTERP => {
                            let weight = self.gaussian_kernel(distance, in_node.get_radius());
                            interp_numerator += weight * in_node.get_intensity();
                            interp_denominator += weight;
                        }
                        InputType::ADDITIVE => {
                            let weight = distance / in_node.get_radius();
                            add_numerator += weight * in_node.get_intensity();
                            add_denominator += weight;
                        }
                        InputType::SUBTRACTIVE => {
                            let weight = distance / in_node.get_radius();
                            add_numerator += weight * (-in_node.get_intensity());
                            add_denominator += weight;
                        }
                    }
                }
            }
        }

        // process if any input nodes have influence over this one
        if interp_denominator > 0.0 || add_denominator > 0.0 {
            let interp_result = if interp_denominator != 0.0 {
                interp_numerator / interp_denominator
            } else {
                0.0
            };

            let add_result = if add_denominator != 0.0 {
                (add_numerator / add_denominator) + interp_result
            } else {
                interp_result
            };

            if add_result > 1.0 {
                1.0
            } else if add_result > 0.02 {
                add_result
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
}

impl Interpolate for GaussianState {
    fn interp(&self, haptic_nodes: &[HapticNode], output: &mut[f32], in_nodes: &[InputNode], settings: &PerDevice) {
        for (i, node) in haptic_nodes.iter().enumerate() {
            // offset = 0.5, intensity = 0.5, input = 1.0 gives 0.75. Offset gives deadzone, and intensity limits final intensity.
            let raw = self.single_node(node, in_nodes);
            output[i] = if raw > 0.0 {
                settings.offset + (1.0 - settings.offset) * settings.intensity * raw
            } else {
                0.0
            };
        }
    }
}
