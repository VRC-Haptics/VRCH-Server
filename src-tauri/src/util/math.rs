use core::f32::consts::PI;

#[inline(always)]
fn rad_to_deg(rad: f32) -> f32 {
    rad * 180.0 / PI
}

/// Minimal 3‑vector with **f32** components for high‑throughput numeric geometry.
#[derive(serde::Serialize, serde::Deserialize, Copy, Clone, Debug, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    #[inline(always)]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline(always)]
    pub fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    #[inline(always)]
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    #[inline(always)]
    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    #[inline(always)]
    pub fn norm(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// Returns true if each component is within `tol` of the corresponding component of `other`.
    pub fn close_to(&self, other: &Vec3, tol: f32) -> bool {
        (self.x - other.x).abs() < tol && (self.y - other.y).abs() < tol && (self.z - other.z).abs() < tol
    }
}

/// Compute the angle in radians around a center axis two points are. Axis is defined by two points.
///
/// * `Some(theta)` where `theta` is in **radians**, 0 ≤ θ ≤ π/2
/// * `None` if either set of points fails to define a plane
#[inline]
pub fn angle_between_points(
    axis_one: Vec3,
    axis_two: Vec3,
    input: Vec3,
    output: Vec3,
) -> Option<f32> {
    // In‑plane edges for plane ABC
    let u1 = axis_one.sub(input); // B – A
    let v1 = axis_two.sub(input); // C – A
                                  // In‑plane edges for plane BCD
    let u2 = axis_one.sub(output); // B – D
    let v2 = axis_two.sub(output); // C – D

    // Normals via cross product
    let n1 = u1.cross(v1);
    let n2 = u2.cross(v2);

    let norm1 = n1.norm();
    let norm2 = n2.norm();

    // Degenerate if any normal has zero length
    if norm1 == 0.0 || norm2 == 0.0 {
        return None;
    }

    // Dot product of normals – abs() to ensure acute angle
    let mut cos_theta = (n1.dot(n2)).abs() / (norm1 * norm2);

    // Clamp to handle tiny numerical overshoots beyond |1|
    if cos_theta > 1.0 {
        cos_theta = 1.0;
    }

    Some(cos_theta.acos()) // radians
}
