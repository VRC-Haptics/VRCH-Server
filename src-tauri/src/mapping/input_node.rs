use crate::util::math::Vec3;

use super::haptic_node::HapticNode;
use super::Id;

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
/// All information needed to compute an OuputNode of any given location
pub struct InputNode {
    /// id Unique to this InputNode
    id: Id,
    /// Contains the standard location and NodeGroup tags for calculating outputs
    pub haptic_node: HapticNode,
    /// The feedback strength at this location
    intensity: f32,
    /// The radius that this node will impact
    radius: f32,
    /// used to identify/modify/remove groups of InputNodes. (tags are not NodeGroups)  
    pub tags: Vec<String>,
    /// how this input node should be interpreted
    pub input_type: InputType,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
/// Describes how an `InputNode` should be used during interpolation.
///
/// Layers are ordered in processing order according to the enum, with the cumulative outputs being passed to the next step.
pub enum InputType {
    /// Default choice, which weights closer values exponentially more.
    ///
    /// Output will not be influenced easily by distant input nodes if there is one close to the output node.
    INTERP,
    /// Additive layers adds the nodes influence into the result of the `InputType::INTERP` step.
    ///
    /// Uses linear scaling based off of the `InputNode.radius`
    ADDITIVE,
    /// Subtractive layers subtracts the nodes influence into the result of the `InputType::INTERP` step.
    ///
    /// Uses linear scaling based off of the `InputNode.radius`
    SUBTRACTIVE,
}

impl InputNode {
    /// Factory for creating InputNode's
    ///
    /// id: Unique id
    ///
    /// node: Fully generated HapticNode in standard space
    ///
    /// tags: Use these to find groups of InputNodes
    ///
    /// **NOTE:** Initializes intensity to 0.0, set the intensity using class functions
    pub fn new(
        node: HapticNode,
        tags: Vec<String>,
        id: Id,
        radius: f32,
        input_type: InputType,
    ) -> InputNode {
        return InputNode {
            id: id,
            haptic_node: node,
            intensity: 0.0,
            radius: radius,
            tags: tags,
            input_type: input_type,
        };
    }

    pub fn always_apply(&self) -> bool {
        self.haptic_node.groups.contains(&super::NodeGroup::All)
    }

    pub fn set_position(&mut self, pos: Vec3) {
        self.haptic_node.x = pos.x;
        self.haptic_node.y = pos.y;
        self.haptic_node.z = pos.z;
    }

    pub fn set_radius(&mut self, radius: f32) {
        self.radius = radius;
    }

    pub fn get_radius(&self) -> f32 {
        self.radius
    }

    /// sets the intensity of this node
    pub fn set_intensity(&mut self, intensity: f32) {
        self.intensity = intensity;
    }

    /// gets the intensity of this node
    pub fn get_intensity(&self) -> f32 {
        self.intensity
    }

    /// Gets our unique ID
    pub fn get_id(&self) -> &Id {
        &self.id
    }
}
