use crate::util::math::Vec3;

use super::BhapticsDevicePositions;

pub fn x6_headset() -> BhapticsDevicePositions {
    let name = "VestFront".to_string();
    let locations: Vec<Vec3> = vec![
        // top -> bottom
        // row 0: Left -> Right
        Vec3::new(-0.0494000018, 1.61039996, 0.101000004),
        Vec3::new(-0.0350000001, 1.61039996, 0.112199999),
        Vec3::new(-0.0168999992, 1.61039996, 0.120999999),
        Vec3::new(0.0494000018, 1.61039996, 0.101000004),
        Vec3::new(0.0350000001, 1.61039996, 0.112199999),
        Vec3::new(0.0168999992, 1.61039996, 0.120999999),
    ];

    BhapticsDevicePositions {
        name: name,
        rows: locations,
    }
}
