use core::f32::consts::PI;
use glam::Vec3;

fn rad_to_deg(rad: f32) -> f32 {
    rad * 180.0 / PI
}

/// Returns true if the dot of the two plane normals is non-negative
/// (i.e. the dihedral angle is within 180°).
/// Returns false if the geometry is degenerate.
#[inline]
pub fn within_half_angle(axis_one: Vec3, axis_two: Vec3, input: Vec3, output: Vec3) -> bool {
    let n1 = (axis_one - input).cross(axis_two - input);
    let n2 = (axis_one - output).cross(axis_two - output);

    // Degenerate check using squared length (avoids sqrt)
    if n1.length_squared() == 0.0 || n2.length_squared() == 0.0 {
        return false;
    }

    n1.dot(n2) >= 0.0
}