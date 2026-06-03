use crate::mapping::{groups::NodeGroup, haptic_node::HapticNode, input_node::InterpolationLayer};
use glam::{Vec3, Vec4};

pub type UuidString = String;

/// Filled with values from a config json file.
/// Provides all information needed to fully define the avatar prefab.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameMap {
    pub schema_version: String,
    pub nodes: Vec<ConfNode>,
    pub identification: ConfIdent,
}

/// Haptic Node information from the game config
/// Contains more information than the default HapticNode to help with locating
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConfNode {
    pub location: Vec3,
    pub interaction_tags: NodeGroup,
    pub parent_bone: TargetBone,
    pub interpolation_layer: InterpolationLayer,
    /// The radius that will be used during interpolation
    pub radius: f32,
    pub inputs: Vec<Input>,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
/// describes an input source.
pub struct Input {
    pub weight: f32,
    pub address: String,
    pub vrc_prefix: String,
    pub external_source: bool,
    pub source: InputType,
    pub layer: InputLayer,
    pub shape: InputShape,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
/// Whether this input source should be interpreted as weight or velocity.
/// Should be moved to mapping if it ends up in there.
pub enum InputType {
    Weight,
    Velocity,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
/// How this input interacts within it's own sources.
/// 
/// Gate(Max((Additive + Subtractive) * Multiplicative, Minimum of Max Layer), Maximum of Min Layer) Clamped to 0.0-1.0 (Overridden by first Override layer)
pub enum InputLayer {
    /// Additive and subtractive combine to create base cumulative values.
    Additive,
    Subtractive,
    /// multiplies by cumulative value
    Multiplicative,
    /// Takes Either cumulative or this value, whichever is bigger
    Max,
    /// Takes Either cumulative or this value, whichever is smaller
    Gate,
    /// Should be use sparingly, could very much so disrupt the outputs.
    Override,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
/// The shape that is used in game, gives us some special ways to interpolate some shapes.
pub enum InputShape {
    Sphere{radius: f32},
    Capsule{radius: f32, length: f32, rotation: Vec4},
    Ray{len: f32, offset: f32},
}

/// Metadata from the json config
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConfIdent {
    pub map_name: String,
    pub map_version: u32,
    pub author_name: String,
    pub author_id: Option<String>,
    pub custom_repository: Option<CustomRepo>,
}

/// where to retrieve different versions of this map
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CustomRepo {
    pub author: String,
    pub map_name: String,
    pub map_author: String,
}

/// The bone that the node is parented to in the prefab.
/// (HumanBodyBones from Unity,)
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum TargetBone {
    Hips,
    LeftUpperLeg,
    RightUpperLeg,
    LeftLowerLeg,
    RightLowerLeg,
    LeftFoot,
    RightFoot,
    Spine,
    Chest,
    Neck,
    Head,
    LeftShoulder,
    RightShoulder,
    LeftUpperArm,
    RightUpperArm,
    LeftLowerArm,
    RightLowerArm,
    LeftHand,
    RightHand,
    LeftToes,
    RightToes,
    LeftEye,
    RightEye,
    Jaw,
    LeftThumbProximal,
    LeftThumbIntermediate,
    LeftThumbDistal,
    LeftIndexProximal,
    LeftIndexIntermediate,
    LeftIndexDistal,
    LeftMiddleProximal,
    LeftMiddleIntermediate,
    LeftMiddleDistal,
    LeftRingProximal,
    LeftRingIntermediate,
    LeftRingDistal,
    LeftLittleProximal,
    LeftLittleIntermediate,
    LeftLittleDistal,
    RightThumbProximal,
    RightThumbIntermediate,
    RightThumbDistal,
    RightIndexProximal,
    RightIndexIntermediate,
    RightIndexDistal,
    RightMiddleProximal,
    RightMiddleIntermediate,
    RightMiddleDistal,
    RightRingProximal,
    RightRingIntermediate,
    RightRingDistal,
    RightLittleProximal,
    RightLittleIntermediate,
    RightLittleDistal,
    UpperChest,
    LastBone,
}
