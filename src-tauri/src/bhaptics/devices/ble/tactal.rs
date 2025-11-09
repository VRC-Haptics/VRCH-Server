use super::bHapticDevice;

use crate::mapping::haptic_node::HapticNode;
use crate::mapping::NodeGroup;
use crate::util::math::Vec3;

pub fn tactal_ble_device() -> bHapticDevice {
    // Original coordinates, unchanged
    const RAW_NODES: [Vec3; 6] = [
        Vec3::new(-0.0494000018, 1.61039996, 0.101000004),
        Vec3::new(-0.0350000001, 1.61039996, 0.112199999),
        Vec3::new(-0.0168999992, 1.61039996, 0.120999999),
        Vec3::new(0.0494000018, 1.61039996, 0.101000004),
        Vec3::new(0.0350000001, 1.61039996, 0.112199999),
        Vec3::new(0.0168999992, 1.61039996, 0.120999999),
    ];

    // Map each Vec3 to a HapticNode with tags
    let nodes = RAW_NODES
        .into_iter()
        .map(|p| HapticNode::new(p, vec![NodeGroup::Head]))
        .collect();

    bHapticDevice {
        ble_name: "Tactal_".to_owned(),
        nodes,
    }
}
