use haptic_core::{glam::{Vec3, Vec4}, mapping::{groups::NodeGroup, input_node::*}};

use crate::{Shape, TagDef};

type Address = String;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct NodeV0 {
    pub loc: Vec3,
    /// list of address, slot pairs.
    pub slots: Vec<(Address, Slot)>,
    pub muted: bool,
    pub radius: f32,
    pub layer: InterpolationLayer,
    pub groups: NodeGroup,
    pub default: f32,
}

#[derive(serde::Deserialize)]
pub struct LocationFileV0 {
    pub locs: Vec<LocationV0>,
    pub tags: Vec<TagDef>,
}

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
/// An in-game location, loosely based on a sever Node.
pub struct LocationV0 {
    pub loc: Vec3,
    pub rot: Vec4,
    pub shapes: Vec<(Address, Shape)>,
    pub tags: Vec<String>
}
