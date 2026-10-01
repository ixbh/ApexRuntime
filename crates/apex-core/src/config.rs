//! Tuning constants.
//!
//! **Every value here is a placeholder until it is backed by a measurement.**
//! Provenance for each lives in `docs/MOVEMENT-MEASUREMENTS.md`; the tag in
//! each doc comment is the short form:
//!
//! * `[source]` – Source SDK / Quake lineage default. Apex descends from it,
//!   but Respawn retuned most of these. Confidence: low.
//! * `[r5-s3]` – appears in R5SDK / Flowstate material describing the Season 3
//!   build. Confidence: medium for S3, low for current Apex.
//! * `[guess]` – picked so the sandbox feels sane. Confidence: none.

use apex_math::{Aabb, Vec3};

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
    /// Phase-1 defaults. Not Apex. See module docs.
    pub const PLACEHOLDER: Self = Self {
        gravity: 750.0,
        jump_height: 56.0,
        auto_jump: false,
        run_speed: 200.0,
        sprint_speed: 300.0,
        crouch_speed: 100.0,
        ground_accel: 10.0,
        friction: 4.0,
        stop_speed: 100.0,
        air_accel: 10.0,
        air_speed_cap: 30.0,
        step_height: 18.0,
        walkable_normal_z: 0.7,
        non_jump_velocity: 140.0,
        max_velocity: 3500.0,
        stand_hull: Aabb::new(Vec3::new(-16.0, -16.0, 0.0), Vec3::new(16.0, 16.0, 72.0)),
        crouch_hull: Aabb::new(Vec3::new(-16.0, -16.0, 0.0), Vec3::new(16.0, 16.0, 48.0)),
        stand_eye_height: 64.0,
        crouch_eye_height: 40.0,
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
