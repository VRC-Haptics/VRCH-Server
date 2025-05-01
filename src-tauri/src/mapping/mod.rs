pub mod global_map;
pub mod haptic_node;
pub mod input_node;
pub mod interp;
pub mod event;

use global_map::GlobalMap;
use haptic_node::HapticNode;
use uuid::Uuid;

use crate::util::math::Vec3;

/// Wrapper function to create a GlobalMap
pub fn create_global_map() -> GlobalMap {
    GlobalMap::new()
}

/// Descriptors for location groups.
/// Allows for segmented Interpolation
#[derive(PartialEq, serde::Deserialize, serde::Serialize, Clone, Debug, strum::EnumIter)]
pub enum NodeGroup {
    Head,
    UpperArmRight,
    UpperArmLeft,
    LowerArmRight,
    LowerArmLeft,
    TorsoRight,
    TorsoLeft,
    TorsoFront,
    TorsoBack,
    UpperLegRight,
    UpperLegLeft,
    LowerLegRight,
    LowerLegLeft,
    FootRight,
    FootLeft,
    /// A meta tag reserved for in-server use only. 
    /// Should not be exported to devices or imported from games.
    All,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
/// Id unique to the node it references.
/// if an Id is equal, it is garunteed to be the same HapticNode, with location in space and tags
pub struct Id(pub String);

impl Id {
    pub fn new() -> Self {Id(Uuid::new_v4().to_string())}
}

impl NodeGroup {
    /// maps node groups into two points defining an axis that runs through the center of the model.
    /// See NodeGroupPoints in standard unity project.
    pub fn to_points(&self) -> (Vec3, Vec3) {
        // helper: flip the sign of the X component for both points
        fn mirror_x(p: (Vec3, Vec3)) -> (Vec3, Vec3) {
            let (a, b) = p;
            (
                Vec3::new(-a.x, a.y, a.z),
                Vec3::new(-b.x, b.y, b.z),
            )
        }

        return match self {
            NodeGroup::TorsoRight
            | NodeGroup::TorsoLeft
            | NodeGroup::TorsoFront
            | NodeGroup::TorsoBack => (Vec3::new(0.,0.735000014,-0.00800000038), 
                Vec3::new(0.,1.43400002,-0.0130000003)),
            NodeGroup::Head => (Vec3::new(0.,1.70700002,0.0529999994), 
                Vec3::new(0.,1.43400002,-0.0130000003)),
            NodeGroup::UpperArmRight => (Vec3::new(0.172999993,1.35599995,-0.0260000005), 
                Vec3::new(0.336199999,1.15139997,-0.0151000004)),
            NodeGroup::LowerArmRight => (Vec3::new(0.336199999,1.14470005,-0.0244999994), 
                Vec3::new(0.4736,0.944899976,0.0469000004)),
            NodeGroup::UpperLegRight => (Vec3::new(0.0689999983,0.921999991,0.00100000005), 
                Vec3::new(0.134000003,0.479000002,-0.0280000009)),
            NodeGroup::LowerLegRight => (Vec3::new(0.134000003,0.479000002,-0.0280000009), 
                Vec3::new(0.173999995,0.0879999995,-0.0729999989)),
            NodeGroup::FootRight => (Vec3::new(0.173999995,0.0879999995,-0.0729999989), 
                Vec3::new(0.226300001,0.0199999996,0.0320000015)),
            NodeGroup::UpperArmLeft  => mirror_x(NodeGroup::UpperArmRight.to_points()),
            NodeGroup::LowerArmLeft  => mirror_x(NodeGroup::LowerArmRight.to_points()),
            NodeGroup::UpperLegLeft  => mirror_x(NodeGroup::UpperLegRight.to_points()),
            NodeGroup::LowerLegLeft  => mirror_x(NodeGroup::LowerLegRight.to_points()),
            NodeGroup::FootLeft      => mirror_x(NodeGroup::FootRight.to_points()),
            NodeGroup::All           => (Vec3::new(0., 0., 0.), Vec3::new(0., 0., 0.))
        }
    }

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
                NodeGroup::UpperArmRight => 1 << 1,
                NodeGroup::UpperArmLeft => 1 << 2,
                NodeGroup::TorsoRight => 1 << 3,
                NodeGroup::TorsoLeft => 1 << 4,
                NodeGroup::TorsoFront => 1 << 5,
                NodeGroup::TorsoBack => 1 << 6,
                NodeGroup::UpperLegRight => 1 << 7,
                NodeGroup::UpperLegLeft => 1 << 8,
                NodeGroup::FootRight => 1 << 9,
                NodeGroup::FootLeft => 1 << 10,
                NodeGroup::LowerArmRight => 1 << 11,
                NodeGroup::LowerArmLeft => 1 << 12,
                NodeGroup::LowerLegRight => 1 << 13,
                NodeGroup::LowerLegLeft => 1 << 14,
                NodeGroup::All => 0,
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
            groups.push(NodeGroup::UpperArmRight);
        }
        if flag & (1 << 2) != 0 {
            groups.push(NodeGroup::UpperArmLeft);
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
            groups.push(NodeGroup::UpperLegRight);
        }
        if flag & (1 << 8) != 0 {
            groups.push(NodeGroup::UpperLegLeft);
        }
        if flag & (1 << 9) != 0 {
            groups.push(NodeGroup::FootRight);
        }
        if flag & (1 << 10) != 0 {
            groups.push(NodeGroup::FootLeft);
        }
        if flag & (1 << 11) != 0 {
            groups.push(NodeGroup::LowerArmRight);
        }
        if flag & (1 << 12) != 0 {
            groups.push(NodeGroup::LowerArmLeft);
        }
        if flag & (1 << 13) != 0 {
            groups.push(NodeGroup::LowerLegRight);
        }
        if flag & (1 << 14) != 0 {
            groups.push(NodeGroup::LowerLegLeft);
        }
        groups
    }
}
