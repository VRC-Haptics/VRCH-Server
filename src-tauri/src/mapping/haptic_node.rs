use std::f32::consts::PI;

use crate::mapping::NodeGroup;
use crate::util::math::{self, Vec3};

/// Struct defining all needed properties for a haptic node.
/// Used for mapping from one haptic model to another.
/// Units are in Meters: Y is vertical, X is aligned with the Right Arm, Z is towards the front.
/// Standard location is zeroed at the reference models feet, directly below the viewpoint.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct HapticNode {
    /// Standard Location in x (meters)
    pub x: f32,
    /// Standard Location in y (meters)
    pub y: f32,
    /// Standard Location in z (meters)
    pub z: f32,
    /// The NodeGroups this node should influence or take influence from
    pub groups: Vec<NodeGroup>,
}

impl HapticNode {
    /// Creates a new haptic node
    pub fn new(pos: Vec3, groups: Vec<NodeGroup>) -> HapticNode {
        HapticNode {
            x: pos.x,
            y: pos.y,
            z: pos.z,
            groups: groups,
        }
    }

    /// Returns Euclidean distance in meters between this and the other HapticNode.
    /// Possibly NaN, needs to be checked
    pub fn dist(&self, other: &HapticNode) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    pub fn to_vec3(&self) -> math::Vec3 {
        Vec3 {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }

    /// Returns true if self and other share any common NodeGroup.
    pub fn interacts(&self, other: &HapticNode) -> bool {
        if other.groups.contains(&NodeGroup::All) || self.groups.contains(&NodeGroup::All) {
            return true;
        }

        for shared_group in &self.groups {
            if other.groups.contains(shared_group) {
                let this = self.to_vec3();
                let that = other.to_vec3();

                let (top, bottom) = shared_group.to_points();
                let angle = math::angle_between_points(top, bottom, this, that)
                    .expect("Unable to get angle");
                if angle <= (PI / 2.) {
                    // only return interactions that are with the 180 deg window
                    return true;
                }
            }
        }

        false
    }

    /// Convert self into an 8-byte array.
    /// * 2 bytes each for x, y, and z (scaled fixed-point)
    /// * 2 bytes for a bitmask representing groups
    pub fn to_bytes(&self) -> [u8; 8] {
        // Moves decimal off of first 3 decimal points (mm precision)
        let scale = 1_000.0;
        let x_fixed = (self.x * scale) as i16;
        let y_fixed = (self.y * scale) as i16;
        let z_fixed = (self.z * scale) as i16;

        // Pack groups into a bitmask.
        let flag = NodeGroup::to_bitflag(&self.groups);

        // Allocate an 8-byte array.
        let mut bytes = [0u8; 8];
        // Use little-endian conversion.
        bytes[0..2].copy_from_slice(&x_fixed.to_le_bytes());
        bytes[2..4].copy_from_slice(&y_fixed.to_le_bytes());
        bytes[4..6].copy_from_slice(&z_fixed.to_le_bytes());
        bytes[6..8].copy_from_slice(&flag.to_le_bytes());
        bytes
    }

    /// Reconstruct a HapticNode from an 8-byte array.
    /// This performs the reverse of `to_bytes`.
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        // Read the fixed-point values using little-endian conversion.
        let x_fixed = i16::from_le_bytes([bytes[0], bytes[1]]);
        let y_fixed = i16::from_le_bytes([bytes[2], bytes[3]]);
        let z_fixed = i16::from_le_bytes([bytes[4], bytes[5]]);
        let flag = u16::from_le_bytes([bytes[6], bytes[7]]);

        // Reverse the scaling (mm precision)
        let scale = 1_000.0;
        HapticNode {
            x: x_fixed as f32 / scale,
            y: y_fixed as f32 / scale,
            z: z_fixed as f32 / scale,
            groups: NodeGroup::from_bitflag(flag),
        }
    }
}
