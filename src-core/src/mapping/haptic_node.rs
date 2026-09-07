use crate::mapping::groups::NodeGroup;
use glam::Vec3;

/// Struct defining all needed properties for a haptic node.
/// Used for mapping from one haptic model to another.
/// Units are in Meters-ish: Y is vertical, X is aligned with the Right Arm, Z is towards the front.
/// Standard location is zeroed at the reference models feet, directly below the viewpoint.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct HapticNode {
    pub loc: Vec3,
    /// The NodeGroups this node should influence or take influence from
    pub groups: NodeGroup,
}

impl HapticNode {
    /// Creates a new haptic node
    pub fn new(loc: Vec3, groups: NodeGroup) -> HapticNode {
        HapticNode {
            loc,
            groups,
        }
    }

    pub fn to_vec3(&self) -> &Vec3 {
        &self.loc
    }

    /// Returns true if self and other share any common NodeGroup.
    pub fn interacts(&self, group: &NodeGroup, loc: &Vec3) -> bool {
        if self.groups.contains(NodeGroup::All) || group.contains(NodeGroup::All) {
            return true;
        }

        let overlap = self.groups.intersection(*group);
        if overlap.is_empty() {
            return false;
        }

        let this = self.to_vec3();
        let that = loc;

        for group in overlap.iter() {
            let Some((top, bottom)) = group.to_points() else {
                continue; // no axis data for this group, skip it
            };
            if within_half_angle(&top, &bottom, this, that) {
                return true;
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
        let x_fixed = (self.loc.x * scale) as i16;
        let y_fixed = (self.loc.y * scale) as i16;
        let z_fixed = (self.loc.z * scale) as i16;

        // Pack groups into a bitmask.
        let flag = self.groups;

        // Allocate an 8-byte array.
        let mut bytes = [0u8; 8];
        // Use little-endian conversion.
        bytes[0..2].copy_from_slice(&x_fixed.to_le_bytes());
        bytes[2..4].copy_from_slice(&y_fixed.to_le_bytes());
        bytes[4..6].copy_from_slice(&z_fixed.to_le_bytes());
        bytes[6..8].copy_from_slice(&flag.bits().to_le_bytes());
        bytes
    }

    /// Reconstruct a HapticNode from an 8-byte array.
    /// This performs the reverse of `to_bytes`.
    pub fn from_bytes(bytes: [u8; 8]) -> Option<Self> {
        // Read the fixed-point values using little-endian conversion.
        let x_fixed = i16::from_le_bytes([bytes[0], bytes[1]]);
        let y_fixed = i16::from_le_bytes([bytes[2], bytes[3]]);
        let z_fixed = i16::from_le_bytes([bytes[4], bytes[5]]);
        let flag = u16::from_le_bytes([bytes[6], bytes[7]]);

        // Reverse the scaling (mm precision)
        const SCALE: f32 = 1_000.0;
        let groups = NodeGroup::from_bits(flag)?;
        Some(HapticNode {
            loc: Vec3::new(
                x_fixed as f32 / SCALE,
                y_fixed as f32 / SCALE,
                z_fixed as f32 / SCALE,
            ),
            groups,
        })
    }
}

/// Calculates whether the nodes are on the same half of the bone.
/// This is used so that nodes on the front and back of legs/torso don't interact.
#[inline]
fn within_half_angle(axis_one: &Vec3, axis_two: &Vec3, input: &Vec3, output: &Vec3) -> bool {
    let n1 = (axis_one - input).cross(axis_two - input);
    let n2 = (axis_one - output).cross(axis_two - output);

    // Degenerate check using squared length (avoids sqrt)
    if n1.length_squared() == 0.0 || n2.length_squared() == 0.0 {
        return false;
    }

    n1.dot(n2) >= 0.0
}
