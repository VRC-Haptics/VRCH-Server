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
    /// used to identify/modify/remove groups of InputNodes. (tags are not NodeGroups)  
    pub tags: Vec<String>,
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
    pub fn new(node: HapticNode, tags: Vec<String>, id: Id) -> InputNode {
        return InputNode {
            id: id,
            haptic_node: node,
            intensity: 0.0,
            tags: tags,
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
