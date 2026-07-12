use crate::{mapping::{Nodes, input_node::InterpolationLayer}, state::PerDevice};

use super::{haptic_node::HapticNode};


#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
/// There is one of these per device node and can be used to cache calculation values.
pub struct InterpState {
    gaussian_merge: f32, // TODO: Maybe look at caching interacted values?
    gaussian_at_edge: f32,
}

impl Default for InterpState {
    fn default() -> Self {
        InterpState { gaussian_merge: 0.002, gaussian_at_edge: 0.05}
    }
}

impl InterpState {
    /// Creates a new Gaussian interpolation instance
    /// Initializes parameters
    pub fn new(merge: f32, at_edge: f32) -> InterpState {
        InterpState {
            gaussian_merge: merge,
            gaussian_at_edge: at_edge,
        }
    }

    /// interpolates values for this devices node positions.
    pub fn interp(&self, haptic_nodes: &[HapticNode], output: &mut[f32], in_nodes: &Nodes, settings: &PerDevice) {
        for (i, node) in haptic_nodes.iter().enumerate() {
            let val = self.single_node(node, in_nodes);
            // calculate device level settings (required)
            output[i] = if val > 0.0 {
                log::trace!("offset: {}, intensity: {}", settings.offset, settings.intensity);
                settings.offset + (settings.intensity - settings.offset) * val
            } else {
                0.0
            };
        }
    }

    /// used in interp function to get a weight between zero and one for the distance between two points.
    #[inline]
    fn gaussian_kernel(&self, distance: f32, max_radius: f32) -> f32 {
        debug_assert!(
            distance >= 0.0 && max_radius > 0.0 && self.gaussian_at_edge > 0.0 && self.gaussian_at_edge < 1.0
        );

        let sigma = max_radius / (-2.0 * self.gaussian_at_edge.ln()).sqrt();

        (-0.5 * (distance / sigma).powi(2)).exp()
    }

    /// returns the straight interpolation for the node.
    /// 
    /// This gets called DeviceNodes * InputNode times PER UPDATE.
    /// Performance is a big deal.
    fn single_node(&self, node: &HapticNode, in_nodes: &Nodes) -> f32 {
        let (mut norm_num, mut norm_den) = (0.0_f32, 0.0_f32);
        let (mut global_num, mut global_den) = (0.0_f32, 0.0_f32);
        let (mut greed_num, mut greed_den) = (0.0_f32, 0.0_f32);

        // collect both transient and streaming nodes weighted values for every input.
        for key in in_nodes.active_streaming.iter().chain(in_nodes.transient.values().flatten()) {
            let Some(in_node) = in_nodes.nodes.get(*key) else {
                log::error!("unable to find node in gaussian kernel");
                continue;
            };

            // if the game node should influence the device node
            if node.interacts(&in_node.groups, &in_node.location) {

                let distance = node.loc.distance(in_node.location);
                match in_node.interpolation_layer {
                    InterpolationLayer::Default => {
                        if !distance.is_nan() && distance < in_node.radius {
                            // weight by gaussian distribution over distance.
                            let weight = self.gaussian_kernel(distance, in_node.radius);
                            norm_num += weight * in_node.value;
                            norm_den += weight;
                        }
                        
                    }
                    InterpolationLayer::Linear => {
                        if !distance.is_nan() && distance < in_node.radius {
                            // weight by distance pctg.
                            let factor =  1. - distance / in_node.radius;
                            let weight = 1.0;
                            norm_num += weight * (in_node.value * factor);
                            norm_den += weight;
                        }
                    }
                    InterpolationLayer::Area => {
                        if !distance.is_nan() && distance < in_node.radius {
                            let weight = 1.0;
                            norm_num += weight * in_node.value;
                            norm_den += weight;
                        }
                    }
                    InterpolationLayer::Greedy => {
                        if !distance.is_nan() && distance < in_node.radius {
                            // weight by distance pctg.
                            let weight = 1f32;
                            greed_num += weight * in_node.value;
                            greed_den += weight;
                        }
                    }
                    InterpolationLayer::Global => {
                        // full weight
                        let weight = 1f32;
                        global_num += weight * in_node.value;
                        global_den += weight;
                    }
                }
            } else {
                continue;
            }
        }

        // presence of global trumps everything else
        if global_den > 0.0 {
            (global_num / global_den).clamp(0.0, 1.0)

        // greedy layer ignores other layers
        } else if greed_den > 0.0 {
            (greed_num / greed_den).clamp(0.0, 1.0)

        // linear and default weight fairly.
        } else if norm_den > 0.0 {
            (norm_num / norm_den).clamp(0.0, 1.0)

        // no nodes
        } else {
            0.0
        }
    }
}
