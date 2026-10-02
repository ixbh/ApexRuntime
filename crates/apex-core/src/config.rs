//! Tuning constants.
//!
//! Values come from `sheets/constants.tsv` (source of truth; generated into
//! [`crate::sheet`] by `build.rs`). Each sheet row records kind, confidence,
//! build and source. **Most are still placeholders, not Apex.** The field
//! tags below are the short form: `[source]` Source/Quake default (low),
//! `[r5-s3]` R5 Season-3 material (medium for S3), `[guess]` (none).

use apex_math::{Aabb, Vec3};

use crate::sheet;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MovementConfig {
    /// u/s². `[source]`/`[r5-s3]` — Titanfall/Apex `sv_gravity` is commonly 750.
    pub gravity: f32,
    /// Apex of a standing jump, in units. `[r5-s3]` — Flowstate notes S3 `player_jumpHeight` = 56.
    pub jump_height: f32,
    /// Hold jump to re-jump on landing. Apex: no. Kept for experiments.
    pub auto_jump: bool,

    /// u/s. `[guess]`
    pub run_speed: f32,
    /// u/s. `[guess]`
    pub sprint_speed: f32,
    /// u/s. `[guess]`
    pub crouch_speed: f32,

    /// Source `sv_accelerate`. `[source]`
    pub ground_accel: f32,
    /// Source `sv_friction`. `[source]`
    pub friction: f32,
    /// Below this speed friction acts as if moving at this speed (Source `sv_stopspeed`). `[source]`
    pub stop_speed: f32,

    /// Source `sv_airaccelerate`. `[source]`
    pub air_accel: f32,
    /// Wish-speed cap used for the air-accel *projection* test (Source's 30). `[source]`
    pub air_speed_cap: f32,

    /// Max height climbed without jumping. `[source]` 18.
    pub step_height: f32,
    /// Minimum ground normal Z to count as walkable (~45.6°). `[source]`
    pub walkable_normal_z: f32,
    /// Upward speed above which you cannot be grounded. `[source]` 140.
    pub non_jump_velocity: f32,
    /// Per-axis speed clamp. `[source]` `sv_maxvelocity`.
    pub max_velocity: f32,

    /// Standing hull, origin at the feet. `[guess]` (Source-like 32×32×72).
    pub stand_hull: Aabb,
    /// Crouched hull, origin at the feet. `[guess]`
    pub crouch_hull: Aabb,
    /// Eye height above the origin, standing / crouched. `[guess]`
    pub stand_eye_height: f32,
    pub crouch_eye_height: f32,
}

impl MovementConfig {
    /// Values from `sheets/constants.tsv` (see [`crate::sheet`]). Most are
    /// still placeholders, not Apex; each row records its confidence.
    pub const PLACEHOLDER: Self = Self {
        gravity: sheet::GRAVITY,
        jump_height: sheet::JUMP_HEIGHT,
        auto_jump: sheet::AUTO_JUMP,
        run_speed: sheet::RUN_SPEED,
        sprint_speed: sheet::SPRINT_SPEED,
        crouch_speed: sheet::CROUCH_SPEED,
        ground_accel: sheet::GROUND_ACCEL,
        friction: sheet::FRICTION,
        stop_speed: sheet::STOP_SPEED,
        air_accel: sheet::AIR_ACCEL,
        air_speed_cap: sheet::AIR_SPEED_CAP,
        step_height: sheet::STEP_HEIGHT,
        walkable_normal_z: sheet::WALKABLE_NORMAL_Z,
        non_jump_velocity: sheet::NON_JUMP_VELOCITY,
        max_velocity: sheet::MAX_VELOCITY,
        stand_hull: Aabb::new(
            Vec3::new(-sheet::HULL_HALF_WIDTH, -sheet::HULL_HALF_WIDTH, 0.0),
            Vec3::new(
                sheet::HULL_HALF_WIDTH,
                sheet::HULL_HALF_WIDTH,
                sheet::STAND_HULL_HEIGHT,
            ),
        ),
        crouch_hull: Aabb::new(
            Vec3::new(-sheet::HULL_HALF_WIDTH, -sheet::HULL_HALF_WIDTH, 0.0),
            Vec3::new(
                sheet::HULL_HALF_WIDTH,
                sheet::HULL_HALF_WIDTH,
                sheet::CROUCH_HULL_HEIGHT,
            ),
        ),
        stand_eye_height: sheet::STAND_EYE_HEIGHT,
        crouch_eye_height: sheet::CROUCH_EYE_HEIGHT,
    };

    /// Vertical launch speed that peaks at exactly `height` under `gravity`.
    pub fn jump_speed(&self, height: f32) -> f32 {
        apex_math::sqrt(2.0 * self.gravity * height)
    }

    pub fn hull(&self, crouched: bool) -> Aabb {
        if crouched {
            self.crouch_hull
        } else {
            self.stand_hull
        }
    }
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self::PLACEHOLDER
    }
}
