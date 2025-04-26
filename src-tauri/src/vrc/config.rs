use crate::mapping::haptic_node::HapticNode;

/// Filled with values from a config json file.
/// Provides all information needed to fully define the avatar prefab.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct GameMap {
    pub nodes: Vec<ConfNode>,
    pub meta: ConfMetadata,
}

/// Haptic Node information from the game config
/// Contains more information than the default HapticNode to help with locating
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct ConfNode {
    pub node_data: HapticNode,
    pub address: String,
    pub is_external_address: bool,
    pub radius: f32,
    pub target_bone: TargetBone,
}

/// Metadata from the json config
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct ConfMetadata {
    pub map_name: String,
    pub map_version: u32,
    pub map_author: String,
}

/// The bone that the node is parented to in the prefab.
/// (HumanBodyBones from Unity,)
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub enum TargetBone {
    /// <summary>
    ///   <para>This is the Hips bone.</para>
    /// </summary>
    Hips,
    /// <summary>
    ///   <para>This is the Left Upper Leg bone.</para>
    /// </summary>
    LeftUpperLeg,
    /// <summary>
    ///   <para>This is the Right Upper Leg bone.</para>
    /// </summary>
    RightUpperLeg,
    /// <summary>
    ///   <para>This is the Left Knee bone.</para>
    /// </summary>
    LeftLowerLeg,
    /// <summary>
    ///   <para>This is the Right Knee bone.</para>
    /// </summary>
    RightLowerLeg,
    /// <summary>
    ///   <para>This is the Left Ankle bone.</para>
    /// </summary>
    LeftFoot,
    /// <summary>
    ///   <para>This is the Right Ankle bone.</para>
    /// </summary>
    RightFoot,
    /// <summary>
    ///   <para>This is the first Spine bone.</para>
    /// </summary>
    Spine,
    /// <summary>
    ///   <para>This is the Chest bone.</para>
    /// </summary>
    Chest,
    /// <summary>
    ///   <para>This is the Neck bone.</para>
    /// </summary>
    Neck,
    /// <summary>
    ///   <para>This is the Head bone.</para>
    /// </summary>
    Head,
    /// <summary>
    ///   <para>This is the Left Shoulder bone.</para>
    /// </summary>
    LeftShoulder,
    /// <summary>
    ///   <para>This is the Right Shoulder bone.</para>
    /// </summary>
    RightShoulder,
    /// <summary>
    ///   <para>This is the Left Upper Arm bone.</para>
    /// </summary>
    LeftUpperArm,
    /// <summary>
    ///   <para>This is the Right Upper Arm bone.</para>
    /// </summary>
    RightUpperArm,
    /// <summary>
    ///   <para>This is the Left Elbow bone.</para>
    /// </summary>
    LeftLowerArm,
    /// <summary>
    ///   <para>This is the Right Elbow bone.</para>
    /// </summary>
    RightLowerArm,
    /// <summary>
    ///   <para>This is the Left Wrist bone.</para>
    /// </summary>
    LeftHand,
    /// <summary>
    ///   <para>This is the Right Wrist bone.</para>
    /// </summary>
    RightHand,
    /// <summary>
    ///   <para>This is the Left Toes bone.</para>
    /// </summary>
    LeftToes,
    /// <summary>
    ///   <para>This is the Right Toes bone.</para>
    /// </summary>
    RightToes,
    /// <summary>
    ///   <para>This is the Left Eye bone.</para>
    /// </summary>
    LeftEye,
    /// <summary>
    ///   <para>This is the Right Eye bone.</para>
    /// </summary>
    RightEye,
    /// <summary>
    ///   <para>This is the Jaw bone.</para>
    /// </summary>
    Jaw,
    /// <summary>
    ///   <para>This is the left thumb 1st phalange.</para>
    /// </summary>
    LeftThumbProximal,
    /// <summary>
    ///   <para>This is the left thumb 2nd phalange.</para>
    /// </summary>
    LeftThumbIntermediate,
    /// <summary>
    ///   <para>This is the left thumb 3rd phalange.</para>
    /// </summary>
    LeftThumbDistal,
    /// <summary>
    ///   <para>This is the left index 1st phalange.</para>
    /// </summary>
    LeftIndexProximal,
    /// <summary>
    ///   <para>This is the left index 2nd phalange.</para>
    /// </summary>
    LeftIndexIntermediate,
    /// <summary>
    ///   <para>This is the left index 3rd phalange.</para>
    /// </summary>
    LeftIndexDistal,
    /// <summary>
    ///   <para>This is the left middle 1st phalange.</para>
    /// </summary>
    LeftMiddleProximal,
    /// <summary>
    ///   <para>This is the left middle 2nd phalange.</para>
    /// </summary>
    LeftMiddleIntermediate,
    /// <summary>
    ///   <para>This is the left middle 3rd phalange.</para>
    /// </summary>
    LeftMiddleDistal,
    /// <summary>
    ///   <para>This is the left ring 1st phalange.</para>
    /// </summary>
    LeftRingProximal,
    /// <summary>
    ///   <para>This is the left ring 2nd phalange.</para>
    /// </summary>
    LeftRingIntermediate,
    /// <summary>
    ///   <para>This is the left ring 3rd phalange.</para>
    /// </summary>
    LeftRingDistal,
    /// <summary>
    ///   <para>This is the left little 1st phalange.</para>
    /// </summary>
    LeftLittleProximal,
    /// <summary>
    ///   <para>This is the left little 2nd phalange.</para>
    /// </summary>
    LeftLittleIntermediate,
    /// <summary>
    ///   <para>This is the left little 3rd phalange.</para>
    /// </summary>
    LeftLittleDistal,
    /// <summary>
    ///   <para>This is the right thumb 1st phalange.</para>
    /// </summary>
    RightThumbProximal,
    /// <summary>
    ///   <para>This is the right thumb 2nd phalange.</para>
    /// </summary>
    RightThumbIntermediate,
    /// <summary>
    ///   <para>This is the right thumb 3rd phalange.</para>
    /// </summary>
    RightThumbDistal,
    /// <summary>
    ///   <para>This is the right index 1st phalange.</para>
    /// </summary>
    RightIndexProximal,
    /// <summary>
    ///   <para>This is the right index 2nd phalange.</para>
    /// </summary>
    RightIndexIntermediate,
    /// <summary>
    ///   <para>This is the right index 3rd phalange.</para>
    /// </summary>
    RightIndexDistal,
    /// <summary>
    ///   <para>This is the right middle 1st phalange.</para>
    /// </summary>
    RightMiddleProximal,
    /// <summary>
    ///   <para>This is the right middle 2nd phalange.</para>
    /// </summary>
    RightMiddleIntermediate,
    /// <summary>
    ///   <para>This is the right middle 3rd phalange.</para>
    /// </summary>
    RightMiddleDistal,
    /// <summary>
    ///   <para>This is the right ring 1st phalange.</para>
    /// </summary>
    RightRingProximal,
    /// <summary>
    ///   <para>This is the right ring 2nd phalange.</para>
    /// </summary>
    RightRingIntermediate,
    /// <summary>
    ///   <para>This is the right ring 3rd phalange.</para>
    /// </summary>
    RightRingDistal,
    /// <summary>
    ///   <para>This is the right little 1st phalange.</para>
    /// </summary>
    RightLittleProximal,
    /// <summary>
    ///   <para>This is the right little 2nd phalange.</para>
    /// </summary>
    RightLittleIntermediate,
    /// <summary>
    ///   <para>This is the right little 3rd phalange.</para>
    /// </summary>
    RightLittleDistal,
    /// <summary>
    ///   <para>This is the Upper Chest bone.</para>
    /// </summary>
    UpperChest,
    /// <summary>
    ///   <para>This is the Last bone index delimiter.</para>
    /// </summary>
    LastBone,
}

impl TargetBone {
    pub fn to_str(&self) -> &str {
        match self {
            TargetBone::Hips => "Hips",
            TargetBone::LeftUpperLeg => "LeftUpperLeg",
            TargetBone::RightUpperLeg => "RightUpperLeg",
            TargetBone::LeftLowerLeg => "LeftLowerLeg",
            TargetBone::RightLowerLeg => "RightLowerLeg",
            TargetBone::LeftFoot => "LeftFoot",
            TargetBone::RightFoot => "RightFoot",
            TargetBone::Spine => "Spine",
            TargetBone::Chest => "Chest",
            TargetBone::Neck => "Neck",
            TargetBone::Head => "Head",
            TargetBone::LeftShoulder => "LeftShoulder",
            TargetBone::RightShoulder => "RightShoulder",
            TargetBone::LeftUpperArm => "LeftUpperArm",
            TargetBone::RightUpperArm => "RightUpperArm",
            TargetBone::LeftLowerArm => "LeftLowerArm",
            TargetBone::RightLowerArm => "RightLowerArm",
            TargetBone::LeftHand => "LeftHand",
            TargetBone::RightHand => "RightHand",
            TargetBone::LeftToes => "LeftToes",
            TargetBone::RightToes => "RightToes",
            TargetBone::LeftEye => "LeftEye",
            TargetBone::RightEye => "RightEye",
            TargetBone::Jaw => "Jaw",
            TargetBone::LeftThumbProximal => "LeftThumbProximal",
            TargetBone::LeftThumbIntermediate => "LeftThumbIntermediate",
            TargetBone::LeftThumbDistal => "LeftThumbDistal",
            TargetBone::LeftIndexProximal => "LeftIndexProximal",
            TargetBone::LeftIndexIntermediate => "LeftIndexIntermediate",
            TargetBone::LeftIndexDistal => "LeftIndexDistal",
            TargetBone::LeftMiddleProximal => "LeftMiddleProximal",
            TargetBone::LeftMiddleIntermediate => "LeftMiddleIntermediate",
            TargetBone::LeftMiddleDistal => "LeftMiddleDistal",
            TargetBone::LeftRingProximal => "LeftRingProximal",
            TargetBone::LeftRingIntermediate => "LeftRingIntermediate",
            TargetBone::LeftRingDistal => "LeftRingDistal",
            TargetBone::LeftLittleProximal => "LeftLittleProximal",
            TargetBone::LeftLittleIntermediate => "LeftLittleIntermediate",
            TargetBone::LeftLittleDistal => "LeftLittleDistal",
            TargetBone::RightThumbProximal => "RightThumbProximal",
            TargetBone::RightThumbIntermediate => "RightThumbIntermediate",
            TargetBone::RightThumbDistal => "RightThumbDistal",
            TargetBone::RightIndexProximal => "RightIndexProximal",
            TargetBone::RightIndexIntermediate => "RightIndexIntermediate",
            TargetBone::RightIndexDistal => "RightIndexDistal",
            TargetBone::RightMiddleProximal => "RightMiddleProximal",
            TargetBone::RightMiddleIntermediate => "RightMiddleIntermediate",
            TargetBone::RightMiddleDistal => "RightMiddleDistal",
            TargetBone::RightRingProximal => "RightRingProximal",
            TargetBone::RightRingIntermediate => "RightRingIntermediate",
            TargetBone::RightRingDistal => "RightRingDistal",
            TargetBone::RightLittleProximal => "RightLittleProximal",
            TargetBone::RightLittleIntermediate => "RightLittleIntermediate",
            TargetBone::RightLittleDistal => "RightLittleDistal",
            TargetBone::UpperChest => "UpperChest",
            TargetBone::LastBone => "LastBone",
        }
    }
}
