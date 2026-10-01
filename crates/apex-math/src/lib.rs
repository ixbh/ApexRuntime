//! Deterministic vector math for ApexRuntime.
//!
//! Conventions (shared by every ApexRuntime crate):
//! * **Z is up.** X/Y form the horizontal plane, matching Source / Respawn / IW4.
//! * **Units are Source units** ("inches"): 1 unit = 0.0254 m. Hosts convert at
//!   the adapter boundary, never inside the core.
//! * **Angles are degrees**, `yaw` about +Z (0 = +X, 90 = +Y), `pitch` positive
//!   looking *down*, as in Source.
//!
//! Trigonometry goes through `libm` rather than `std` so that results are the
//! same bits on every platform/toolchain; `sqrt` is IEEE-exact either way.

use core::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const X: Self = Self::new(1.0, 0.0, 0.0);
    pub const Y: Self = Self::new(0.0, 1.0, 0.0);
    pub const Z: Self = Self::new(0.0, 0.0, 1.0);

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub const fn splat(v: f32) -> Self {
        Self::new(v, v, v)
    }

    #[inline]
    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    #[inline]
    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    #[inline]
    pub fn length_sq(self) -> f32 {
        self.dot(self)
    }

    #[inline]
    pub fn length(self) -> f32 {
        sqrt(self.length_sq())
    }

    /// Length of the horizontal (XY) component.
    #[inline]
    pub fn length_2d(self) -> f32 {
        sqrt(self.x * self.x + self.y * self.y)
    }

    /// The vector with Z zeroed.
    #[inline]
    pub fn horizontal(self) -> Self {
        Self::new(self.x, self.y, 0.0)
    }

    /// Normalized copy, or zero if the vector is (near) zero.
    #[inline]
    pub fn normalize_or_zero(self) -> Self {
        let len = self.length();
        if len > 1e-6 { self / len } else { Self::ZERO }
    }

    /// Normalizes in place and returns the original length (Source `VectorNormalize`).
    #[inline]
    pub fn normalize_with_len(&mut self) -> f32 {
        let len = self.length();
        if len > 1e-6 {
            *self = *self / len;
        } else {
            *self = Self::ZERO;
        }
        len
    }

    #[inline]
    pub fn mul_add(self, scale: f32, o: Self) -> Self {
        Self::new(
            self.x + o.x * scale,
            self.y + o.y * scale,
            self.z + o.z * scale,
        )
    }

    #[inline]
    pub fn min(self, o: Self) -> Self {
        Self::new(self.x.min(o.x), self.y.min(o.y), self.z.min(o.z))
    }

    #[inline]
    pub fn max(self, o: Self) -> Self {
        Self::new(self.x.max(o.x), self.y.max(o.y), self.z.max(o.z))
    }

    #[inline]
    pub fn lerp(self, o: Self, t: f32) -> Self {
        self + (o - self) * t
    }

    #[inline]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    #[inline]
    pub fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    #[inline]
    pub fn from_array(a: [f32; 3]) -> Self {
        Self::new(a[0], a[1], a[2])
    }

    #[inline]
    pub fn component(self, axis: usize) -> f32 {
        match axis {
            0 => self.x,
            1 => self.y,
            _ => self.z,
        }
    }
}

impl Add for Vec3 {
    type Output = Self;
    #[inline]
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl AddAssign for Vec3 {
    #[inline]
    fn add_assign(&mut self, o: Self) {
        *self = *self + o;
    }
}
impl Sub for Vec3 {
    type Output = Self;
    #[inline]
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl SubAssign for Vec3 {
    #[inline]
    fn sub_assign(&mut self, o: Self) {
        *self = *self - o;
    }
}
impl Mul<f32> for Vec3 {
    type Output = Self;
    #[inline]
    fn mul(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}
impl MulAssign<f32> for Vec3 {
    #[inline]
    fn mul_assign(&mut self, s: f32) {
        *self = *self * s;
    }
}
impl Div<f32> for Vec3 {
    type Output = Self;
    #[inline]
    fn div(self, s: f32) -> Self {
        Self::new(self.x / s, self.y / s, self.z / s)
    }
}
impl Neg for Vec3 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

#[inline]
pub fn sqrt(v: f32) -> f32 {
    libm::sqrtf(v)
}

#[inline]
pub fn sin(v: f32) -> f32 {
    libm::sinf(v)
}

#[inline]
pub fn cos(v: f32) -> f32 {
    libm::cosf(v)
}

#[inline]
pub fn deg_to_rad(d: f32) -> f32 {
    d * (core::f32::consts::PI / 180.0)
}

/// Wraps an angle in degrees to `[-180, 180)`.
#[inline]
pub fn wrap_degrees(d: f32) -> f32 {
    let mut a = libm::fmodf(d + 180.0, 360.0);
    if a < 0.0 {
        a += 360.0;
    }
    a - 180.0
}

/// Source-style `AngleVectors` for a (pitch, yaw) pair in degrees.
/// Returns `(forward, right, up)`; roll is assumed zero.
pub fn angle_vectors(pitch_deg: f32, yaw_deg: f32) -> (Vec3, Vec3, Vec3) {
    let (sp, cp) = (sin(deg_to_rad(pitch_deg)), cos(deg_to_rad(pitch_deg)));
    let (sy, cy) = (sin(deg_to_rad(yaw_deg)), cos(deg_to_rad(yaw_deg)));
    let forward = Vec3::new(cp * cy, cp * sy, -sp);
    let right = Vec3::new(sy, -cy, 0.0);
    let up = Vec3::new(sp * cy, sp * sy, cp);
    (forward, right, up)
}

/// Horizontal basis from yaw only: `(forward, right)` with Z = 0, unit length.
/// Movement wish directions are built from this so looking up/down never
/// changes ground speed.
pub fn yaw_basis(yaw_deg: f32) -> (Vec3, Vec3) {
    let (sy, cy) = (sin(deg_to_rad(yaw_deg)), cos(deg_to_rad(yaw_deg)));
    (Vec3::new(cy, sy, 0.0), Vec3::new(sy, -cy, 0.0))
}

/// Axis-aligned bounding box (used for player hulls).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Aabb {
    pub mins: Vec3,
    pub maxs: Vec3,
}

impl Aabb {
    pub const fn new(mins: Vec3, maxs: Vec3) -> Self {
        Self { mins, maxs }
    }

    pub fn translated(self, by: Vec3) -> Self {
        Self::new(self.mins + by, self.maxs + by)
    }

    pub fn overlaps(self, o: Self) -> bool {
        self.mins.x < o.maxs.x
            && self.maxs.x > o.mins.x
            && self.mins.y < o.maxs.y
            && self.maxs.y > o.mins.y
            && self.mins.z < o.maxs.z
            && self.maxs.z > o.mins.z
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaw_basis_is_orthonormal_and_right_handed_for_source() {
        let (f, r) = yaw_basis(0.0);
        assert!((f - Vec3::X).length() < 1e-6);
        // Source: yaw 0 looks down +X, "right" is -Y.
        assert!((r - Vec3::new(0.0, -1.0, 0.0)).length() < 1e-6);
        let (f, _) = yaw_basis(90.0);
        assert!((f - Vec3::Y).length() < 1e-6);
    }

    #[test]
    fn angle_vectors_pitch_down_is_negative_z() {
        let (f, _, _) = angle_vectors(90.0, 0.0);
        assert!(f.z < -0.999);
    }

    #[test]
    fn wrap_degrees_range() {
        assert_eq!(wrap_degrees(190.0), -170.0);
        assert_eq!(wrap_degrees(-190.0), 170.0);
        assert_eq!(wrap_degrees(0.0), 0.0);
    }
}
