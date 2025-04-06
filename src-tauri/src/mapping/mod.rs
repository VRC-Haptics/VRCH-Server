pub mod haptic_node;
pub mod input_node;
pub mod global_map;
pub mod interp;
use global_map::GlobalMap;
use haptic_node::HapticNode;

/// Wrapper function to create a GlobalMap
pub fn create_global_map() -> GlobalMap {
    GlobalMap::new()
}

/// The types of modifiers that the global map supports for modifying input
#[derive(PartialEq, serde::Deserialize, serde::Serialize, Clone, Debug)]
pub enum GlobalModifier {
    /// Percentage to apply to all haptics (1 = no change)
    Intensity(f32),
}

/// Descriptors for location groups.
/// Allows for segmented Interpolation
#[derive(PartialEq, serde::Deserialize, serde::Serialize, Clone, Debug)]
pub enum NodeGroup {
    Head,
    ArmRight,
    ArmLeft,
    TorsoRight,
    TorsoLeft,
    TorsoFront,
    TorsoBack,
    LegRight,
    LegLeft,
    FootRight,
    FootLeft,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
/// Id unique to the node it references.
/// if an Id is equal, it is garunteed to be the same HapticNode, with location in space and tags
pub struct Id(pub String);

impl NodeGroup {
    /// Given a string containing at least 2 raw bytes, interpret the first two bytes as
    /// a little-endian u16 bitflag and convert that into a Vec<NodeGroup>.
    pub fn parse_from_str(s: &str) -> Vec<NodeGroup> {
        let bytes = s.as_bytes();
        if bytes.len() < 2 {
            // Not enough data; return an empty vector
            return Vec::new();
        }
        let flag = u16::from_le_bytes([bytes[0], bytes[1]]);
        Self::from_bitflag(flag)
    }

    /// Converts a slice of NodeGroup into a bitflag.
    pub fn to_bitflag(groups: &[NodeGroup]) -> u16 {
        let mut flag: u16 = 0;
        for group in groups {
            flag |= match group {
                NodeGroup::Head => 1 << 0,
                NodeGroup::ArmRight => 1 << 1,
                NodeGroup::ArmLeft => 1 << 2,
                NodeGroup::TorsoRight => 1 << 3,
                NodeGroup::TorsoLeft => 1 << 4,
                NodeGroup::TorsoFront => 1 << 5,
                NodeGroup::TorsoBack => 1 << 6,
                NodeGroup::LegRight => 1 << 7,
                NodeGroup::LegLeft => 1 << 8,
                NodeGroup::FootRight => 1 << 9,
                NodeGroup::FootLeft => 1 << 10,
            }
        }
        flag
    }

    /// Converts a bitflag back into a vector of NodeGroup variants.
    pub fn from_bitflag(flag: u16) -> Vec<NodeGroup> {
        let mut groups = Vec::new();
        if flag & (1 << 0) != 0 {
            groups.push(NodeGroup::Head);
        }
        if flag & (1 << 1) != 0 {
            groups.push(NodeGroup::ArmRight);
        }
        if flag & (1 << 2) != 0 {
            groups.push(NodeGroup::ArmLeft);
        }
        if flag & (1 << 3) != 0 {
            groups.push(NodeGroup::TorsoRight);
        }
        if flag & (1 << 4) != 0 {
            groups.push(NodeGroup::TorsoLeft);
        }
        if flag & (1 << 5) != 0 {
            groups.push(NodeGroup::TorsoFront);
        }
        if flag & (1 << 6) != 0 {
            groups.push(NodeGroup::TorsoBack);
        }
        if flag & (1 << 7) != 0 {
            groups.push(NodeGroup::LegRight);
        }
        if flag & (1 << 8) != 0 {
            groups.push(NodeGroup::LegLeft);
        }
        if flag & (1 << 9) != 0 {
            groups.push(NodeGroup::FootRight);
        }
        if flag & (1 << 10) != 0 {
            groups.push(NodeGroup::FootLeft);
        }
        groups
    }
}
