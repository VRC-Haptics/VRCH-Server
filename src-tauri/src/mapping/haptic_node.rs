use crate::mapping::NodeGroup;

/// Struct defining all needed properties for a haptic node.
/// Used for mapping from one haptic model to another.
/// Units are in Meters: Z is vertical, X is aligned with the Left Arm, Y is towards the front.
/// Standard location is zeroed at the reference models feet, directly below the viewpoint.
#[derive(serde::Deserialize, serde::Serialize)]
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
    /// Returns Euclidean distance in meters between this and the other HapticNode.
    /// Possibly NaN, needs to be checked
    pub fn dist(&self, other: &HapticNode) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Returns true if self and other share any common NodeGroup.
    pub fn interacts(&self, other: &HapticNode) -> bool {
        //TODO: Better filter this
        // Iterate over the smaller group vector for efficiency.
        if self.groups.len() <= other.groups.len() {
            for group in &self.groups {
                if other.groups.contains(group) {
                    return true;
                }
            }
        } else {
            for group in &other.groups {
                if self.groups.contains(group) {
                    return true;
                }
            }
        }
        false
    }
}
