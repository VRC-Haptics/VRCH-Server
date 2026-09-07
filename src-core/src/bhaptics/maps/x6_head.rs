use glam::Vec3;

use super::BhapticsDevicePositions;

pub fn x6_headset() -> BhapticsDevicePositions {
    let name = "VestFront".to_string();
    let locations: Vec<Vec3> = vec![
        // top -> bottom
        // row 0: Left -> Right
        Vec3::new(-0.049_4, 1.610_4, 0.101),
        Vec3::new(-0.035, 1.610_4, 0.112_2),
        Vec3::new(-0.016_9, 1.610_4, 0.121),
        Vec3::new(0.049_4, 1.610_4, 0.101),
        Vec3::new(0.035, 1.610_4, 0.112_2),
        Vec3::new(0.016_9, 1.610_4, 0.121),
    ];

    BhapticsDevicePositions {
        name,
        rows: locations,
    }
}
