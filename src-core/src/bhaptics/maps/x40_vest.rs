use glam::Vec3;

use super::BhapticsDevicePositions;

/// returns the back vest device positions in haptic space
pub fn x40_vest_front() -> BhapticsDevicePositions {
    let name = "VestFront".to_string();
    let locations: Vec<Vec3> = vec![
        // top -> bottom
        // row 0: Left -> Right
        Vec3::new(-0.151_1, 1.29, 0.076_4),
        Vec3::new(-0.051, 1.29, 0.101),
        Vec3::new(0.051, 1.29, 0.101),
        Vec3::new(0.151_1, 1.29, 0.076_4),
        // row 1
        Vec3::new(-0.1426, 1.19, 0.059),
        Vec3::new(-0.049, 1.19, 0.115_7),
        Vec3::new(0.049, 1.19, 0.115_7),
        Vec3::new(0.1426, 1.19, 0.059),
        // row 2
        Vec3::new(-0.133_1, 1.105, 0.049_2),
        Vec3::new(-0.048_099_995, 1.105, 0.121_2),
        Vec3::new(0.048_099_995, 1.105, 0.121_2),
        Vec3::new(0.133_1, 1.105, 0.049_2),
        // row 3
        Vec3::new(-0.133_1, 1.02, 0.049_2),
        Vec3::new(-0.048_099_995, 1.02, 0.114_2),
        Vec3::new(0.048_099_995, 1.02, 0.114_2),
        Vec3::new(0.133_1, 1.02, 0.049_2),
        // row 4
        Vec3::new(-0.14, 0.927, 0.058),
        Vec3::new(-0.051, 0.927, 0.094),
        Vec3::new(0.051, 0.927, 0.094),
        Vec3::new(0.14, 0.927, 0.058),
    ];

    BhapticsDevicePositions {
        name,
        rows: locations,
    }
}

/// returns the back vest device positions in haptic space
pub fn x40_vest_back() -> BhapticsDevicePositions {
    let name = "VestBack".to_string();
    let locations: Vec<Vec3> = vec![
        // row 0: Left -> Right
        Vec3::new(-0.151_9, 1.29, -0.126_199_99),
        Vec3::new(-0.051, 1.29, -0.12619999),
        Vec3::new(0.051, 1.29, -0.126_199_99),
        Vec3::new(0.151_9, 1.29, -0.126_199_99),
        // row 1
        Vec3::new(-0.1426, 1.19, -0.09),
        Vec3::new(-0.049, 1.19, -0.111_5),
        Vec3::new(0.049, 1.19, -0.111_5),
        Vec3::new(0.1426, 1.19, -0.09),
        // row 2
        Vec3::new(-0.133_1, 1.105, -0.067),
        Vec3::new(-0.048_099_995, 1.105, -0.106),
        Vec3::new(0.048_099_99, 1.105, -0.106),
        Vec3::new(0.133_1, 1.105, -0.067),
        // row 3
        Vec3::new(-0.133_1, 1.02, -0.048),
        Vec3::new(-0.048_099_995, 1.02, -0.080_999_99),
        Vec3::new(0.048_099_99, 1.02, -0.080_999_99),
        Vec3::new(0.133_1, 1.02, -0.048_000_004),
        // row 4
        Vec3::new(-0.14, 0.941, -0.053_000_003),
        Vec3::new(-0.051, 0.941, -0.092_000_01),
        Vec3::new(0.051, 0.941, -0.092_000_01),
        Vec3::new(0.14, 0.941, -0.053),
    ];

    BhapticsDevicePositions {
        name,
        rows: locations,
    }
}
