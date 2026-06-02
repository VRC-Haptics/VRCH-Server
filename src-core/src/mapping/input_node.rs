use std::time::Instant;
use glam::Vec3;
use smallvec::SmallVec;

use crate::mapping::NodeKey;
use crate::mapping::groups::NodeGroup;
use crate::vrc::config::{InputLayer, InputType};

const DEFAULT_NODE_SLOTS: usize = 2;

/// Points to an input slot, within a node, within an interpolation layer.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SlotKey {
    pub node: NodeKey,
    pub slot_idx: u8,
}

impl SlotKey {
    pub fn new(node: NodeKey, idx: u8) -> Self {
        SlotKey { node, slot_idx: idx }
    }
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct Slot {
    pub muted: bool,
    pub source: InputType,
    pub layer: InputLayer,
    pub weight: f32,
    pub value: f32,
    #[serde(skip)]
    pub history: SmallVec<[(f32, Instant); 3]>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
/// Represents information at a point in space that will go into computing outputs.
/// 
/// To have a Greedy layer drive multiple devices it must have the ALL group tag present. 
pub struct InputNode {
    pub muted: bool,
    pub location: Vec3,
    /// A slot is an single input value.
    #[specta(type = Vec<Slot>)]
    pub slots: SmallVec<[Slot; DEFAULT_NODE_SLOTS]>,
    pub radius: f32,
    /// purely for convenience, should be determined by the map.
    pub interpolation_layer: InterpolationLayer,
    pub groups: NodeGroup,
    /// The most recently calculated output value of this node.
    pub value: f32,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[repr(u8)]
/// Describes how this node should affect the layers. 
pub enum InterpolationLayer {
    /// Grabs any nodes within it's radius,forcefully drives it at this value.
    Greedy,
    /// Exponentially weights nodes within it's radius, nodes at 0 pull to zero.
    #[default]
    Default,
    /// Weights nodes according to their proximity to this node
    Linear,
    /// Drives **all** nodes exactly at output without regard to distance.
    Global,
}

impl InputNode {
    /// Factory for creating InputNode's
    pub fn new(
        location: Vec3,
        groups: NodeGroup,
        layer: InterpolationLayer,
        slots: SmallVec<[Slot; DEFAULT_NODE_SLOTS]>,
        radius: f32,
    ) -> InputNode {
        return InputNode {
            muted: false,
            location,
            slots,
            radius,
            interpolation_layer: layer,
            groups,
            value: 0.0,
        };
    }

    pub const fn always_apply(&self) -> bool {
        self.groups.intersects(NodeGroup::All)
    }

    pub fn set_position(&mut self, pos: Vec3) {
        self.location.x = pos.x;
        self.location.y = pos.y;
        self.location.z = pos.z;
    }

    pub fn mute(&mut self, val: bool) {
        self.muted = val;
    }

    pub fn set_radius(&mut self, radius: f32) {
        self.radius = radius;
    }

    pub fn get_radius(&self) -> f32 {
        self.radius
    }
}
