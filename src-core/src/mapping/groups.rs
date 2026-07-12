
use bitflags::{bitflags, bitflags_match};
use glam::Vec3;

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeGroup(u16);

bitflags! {
    impl NodeGroup: u16 {
        const Head          = 0b0000000000000001;
        const UpperArmRight = 0b0000000000000010;
        const UpperArmLeft  = 0b0000000000000100;
        const LowerArmRight = 0b0000000000001000;
        const LowerArmLeft  = 0b0000000000010000;
        const TorsoRight    = 0b0000000000100000;
        const TorsoLeft     = 0b0000000001000000;
        const TorsoFront    = 0b0000000010000000;
        const TorsoBack     = 0b0000000100000000;
        const UpperLegRight = 0b0000001000000000;
        const UpperLegLeft  = 0b0000010000000000;
        const LowerLegRight = 0b0000100000000000;
        const LowerLegLeft  = 0b0001000000000000;
        const FootRight     = 0b0010000000000000;
        const FootLeft      = 0b0100000000000000;
        const All           = 0b1000000000000000;
    }
}

impl serde::Serialize for NodeGroup {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // emits each set flag's name as an array element: ["Head", "TorsoFront"]
        s.collect_seq(self.iter_names().map(|(name, _)| name))
    }
}

impl<'de> serde::Deserialize<'de> for NodeGroup {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut flags = NodeGroup::empty();
        for name in Vec::<String>::deserialize(d)? {
            flags |= NodeGroup::from_name(&name).ok_or_else(|| {
                serde::de::Error::custom(format!("unknown NodeGroup flag: {name}"))
            })?;
        }
        Ok(flags)
    }
}

impl NodeGroup {
    /// Returns the (top, bottom) axis points for a single NodeGroup flag.
    /// Returns None for `All` or any combined/empty set.
    pub fn to_points(&self) -> Option<(Vec3, Vec3)> {
        const fn mirror_x(p: Option<(Vec3, Vec3)>) -> Option<(Vec3, Vec3)> {
            match p {
                Some((a, b)) => Some((
                    Vec3::new(-a.x, a.y, a.z),
                    Vec3::new(-b.x, b.y, b.z),
                )),
                None => None,
            }
        }

        // NOTE: bitflags_match! matches on EXACT equality, not match-style
        // or-patterns. Each arm must therefore be a single flag — never
        // `A | B | ...`, which would only match the exact combined bit set.
        // to_points is contracted to be called per-flag (see interacts).
        bitflags_match!(*self, {
            NodeGroup::Head => Some((
                Vec3::new(0., 1.70700002, 0.0529999994),
                Vec3::new(0., 1.43400002, -0.0130000003),
            )),

            // All four torso faces share the same central vertical axis;
            // front/back and left/right are resolved by within_half_angle.
            NodeGroup::TorsoRight => Some((
                Vec3::new(0., 0.735000014, -0.00800000038),
                Vec3::new(0., 1.43400002, -0.0130000003),
            )),
            NodeGroup::TorsoLeft => Some((
                Vec3::new(0., 0.735000014, -0.00800000038),
                Vec3::new(0., 1.43400002, -0.0130000003),
            )),
            NodeGroup::TorsoFront => Some((
                Vec3::new(0., 0.735000014, -0.00800000038),
                Vec3::new(0., 1.43400002, -0.0130000003),
            )),
            NodeGroup::TorsoBack => Some((
                Vec3::new(0., 0.735000014, -0.00800000038),
                Vec3::new(0., 1.43400002, -0.0130000003),
            )),

            NodeGroup::UpperArmRight => Some((
                Vec3::new(0.172999993, 1.35599995, -0.0260000005),
                Vec3::new(0.336199999, 1.15139997, -0.0151000004),
            )),

            NodeGroup::UpperLegRight => Some((
                Vec3::new(0.0689999983, 0.921999991, 0.00100000005),
                Vec3::new(0.134000003, 0.479000002, -0.0280000009),
            )),

            NodeGroup::LowerLegRight => Some((
                Vec3::new(0.134000003, 0.479000002, -0.0280000009),
                Vec3::new(0.173999995, 0.0879999995, -0.0729999989),
            )),

            NodeGroup::FootRight => Some((
                Vec3::new(0.173999995, 0.0879999995, -0.0729999989),
                Vec3::new(0.226300001, 0.0199999996, 0.0320000015),
            )),

            NodeGroup::UpperArmLeft => mirror_x(NodeGroup::UpperArmRight.to_points()),
            NodeGroup::LowerArmLeft => mirror_x(NodeGroup::LowerArmRight.to_points()),
            NodeGroup::UpperLegLeft => mirror_x(NodeGroup::UpperLegRight.to_points()),
            NodeGroup::LowerLegLeft => mirror_x(NodeGroup::LowerLegRight.to_points()),
            NodeGroup::FootLeft     => mirror_x(NodeGroup::FootRight.to_points()),

            _ => None,
        })
    }
}