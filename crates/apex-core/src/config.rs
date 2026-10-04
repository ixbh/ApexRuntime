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
    /// u/s². Measured 750.
    pub gravity: f32,
    /// Jump height setting: launch speed is √(2·g·h). Measured 56 (actual rise
    /// is lower and fps-dependent, see `step`).
    pub jump_height: f32,
    /// Hold jump to re-jump on landing. Apex: no. Kept for experiments.
    pub auto_jump: bool,

    /// u/s. Measured.
    pub run_speed: f32,
    /// u/s. Measured.
    pub sprint_speed: f32,
    /// u/s. Measured.
    pub crouch_speed: f32,

    /// Net ground acceleration (u/s²) from the end of the start kick up to run speed.
    pub ground_accel: f32,
    /// Net acceleration (u/s²) from standstill until `ground_kick_until_frac × run_speed`.
    pub ground_kick_accel: f32,
    pub ground_kick_until_frac: f32,
    /// Net acceleration (u/s²) above run speed (run → sprint ramp).
    pub sprint_ramp_accel: f32,
    /// Source `sv_friction`; with `stop_speed` gives Apex's constant 1250 u/s² stop.
    pub friction: f32,
    /// Source `sv_stopspeed`.
    pub stop_speed: f32,
    /// Friction multiplier while fully crouched (not sliding).
    pub crouch_friction_scale: f32,

    /// Seconds to fully crouch / fully stand (ground only; air is instant).
    pub crouch_in_time: f32,
    pub crouch_out_time: f32,

    /// Slide: speed added on entry (if off cooldown), capped at `slide_boost_cap`.
    pub slide_boost: f32,
    pub slide_boost_cap: f32,
    pub slide_boost_cooldown: f32,
    /// Slide deceleration = const + linear·speed (u/s²).
    pub slide_decel_const: f32,
    pub slide_decel_linear: f32,
    /// Slide ends below this speed.
    pub slide_end_speed: f32,
    /// Crouching at or above this speed starts a slide.
    pub slide_start_min_speed: f32,

    /// Lurch strength k (fraction of the way the velocity direction turns toward
    /// the wish direction per new airborne press), as a function of time in air:
    /// `start` at takeoff → `peak` at `peak_time` → 0 at `window`.
    pub lurch_strength_start: f32,
    pub lurch_strength: f32,
    pub lurch_peak_time: f32,
    pub lurch_window: f32,
    /// No lurch above this horizontal speed (S21 `jump_grace_cutoff_speed`).
    pub lurch_speed_cutoff: f32,

    /// Jump fatigue: after landing from a jump, jump *height* is
    /// `jump_fatigue_min_frac` until `jump_fatigue_start` seconds on the ground,
    /// then recovers linearly to full at `jump_fatigue_end`.
    pub jump_fatigue_min_frac: f32,
    pub jump_fatigue_start: f32,
    pub jump_fatigue_end: f32,
    /// Launch-speed multiplier for jumps out of a slide.
    pub slide_jump_mult: f32,
    /// An unfatigued jump out of a slide raises horizontal speed to at least this.
    pub slide_jump_min_speed: f32,
    /// Bunnyhop penalty: a jump within `bhop_skip_time` of landing cuts
    /// horizontal speed above `sprint_speed` by up to `bhop_speed_penalty`.
    pub bhop_skip_time: f32,
    /// Coyote time: a jump still works this long after leaving the ground
    /// without one.
    pub coyote_time: f32,
    pub bhop_speed_penalty: f32,

    /// Mantle: duration = base + per_unit × ledge height above the feet.
    pub mantle_duration_base: f32,
    pub mantle_duration_per_unit: f32,
    /// Horizontal travel onto the ledge.
    pub mantle_forward: f32,
    /// Highest ledge (above the feet) that can be mantled; from standing on the ground.
    pub mantle_max_height: f32,
    pub mantle_ground_max_height: f32,
    /// Classic superglide: jump in the last `superglide_window` seconds of a
    /// mantle, crouch within `superglide_crouch_ticks` ticks after it.
    pub superglide_window: f32,
    pub superglide_crouch_ticks: f32,
    /// Last part of a mantle (the pull): a jump here but before the
    /// superglide window does nothing; earlier (the grab) it hops off.
    pub mantle_pull_time: f32,
    /// Ledges up to this height are pull-only (no grab to hop out of).
    pub mantle_grab_min_height: f32,
    /// S21 mantle boost (late jump without crouch): exit speed multiplier and jump height.
    pub mantle_boost_mult: f32,
    pub mantle_boost_jump_height: f32,
    pub mantle_boost_enabled: bool,

    /// Wall climb: attaches when vz falls to `climb_attach_vz`, accelerates to
    /// the peak, then decays exponentially; ends with a push at the climb-space cutoff.
    pub climb_attach_vz: f32,
    pub climb_accel: f32,
    pub climb_peak_speed: f32,
    pub climb_decay_rate: f32,
    pub climb_end_push_up: f32,
    pub climb_end_push_out: f32,
    /// Jump off a wall: push along the normal (and along the A/D input).
    pub wall_jump_push: f32,
    /// Distance ahead in which a wall counts as touched.
    pub wall_probe_dist: f32,
    /// Pushing into a wall: wish·(−normal) above this, or moving into it faster
    /// than `wall_push_min_speed`.
    pub wall_push_min_dot: f32,
    pub wall_push_min_speed: f32,
    /// Sideways (along-the-wall) speed carried through a mantle decays at this
    /// rate unless A/D is held that way.
    pub mantle_side_decay: f32,
    /// Sideways climb (wallrun): along-the-wall target speed with A/D, and
    /// the (lower) one during an upward W climb; approached at `climb_side_accel`.
    pub climb_side_speed: f32,
    pub climb_up_side_speed: f32,
    pub climb_side_accel: f32,
    /// A lurch changes horizontal velocity by at most this × sprint speed
    /// (+ one tick of air acceleration); the rest of the turn is lost.
    pub lurch_max_delta_frac: f32,
    /// No-mantle hang while rising: dvz/dt = −decel + drag·vz.
    pub no_mantle_vz_decel: f32,
    pub no_mantle_vz_drag: f32,
    pub climb_up_side_accel: f32,
    /// Sideways climb while rising: dvz/dt = −gravity + drag·vz.
    pub climb_side_vz_drag: f32,
    /// The climb speed ceiling starts decaying at this time (or at the peak).
    pub climb_peak_time: f32,
    /// Climbing slower than `climb_nonupward_min_vz` detaches once the climb
    /// is older than `climb_nonupward_time`.
    pub climb_nonupward_time: f32,
    pub climb_nonupward_min_vz: f32,
    /// View must face the wall: forward·(−normal) at least this (45.57°).
    pub climb_look_min_dot: f32,
    /// Push into the wall each climb tick so the climb hugs the surface.
    pub climb_suction: f32,
    /// A climb becomes a mantle only with input this much into the wall (50°).
    pub mantle_input_min_dot: f32,
    /// Climb space (wiki): measured from the baseline (feet at the last ground
    /// contact). Climbing ends at `climb_space_height` above it or
    /// `climb_attach_offset` above the attach point; zones decide wall jumps.
    pub climb_space_height: f32,
    pub climb_attach_offset: f32,
    pub climb_mini_zone_top: f32,
    pub climb_green_zone_top: f32,
    /// Climb space drop on a detach (double for green/neutral wall jumps).
    pub climb_detach_penalty: f32,
    /// Reattach on the same wall needs dropping below the last attach point;
    /// a wall with normal·last < this counts as a different wall.
    pub climb_reattach_min_dot: f32,
    /// Green-zone wall jump: vz = sqrt(a − b·height above the baseline).
    pub wall_bounce_a: f32,
    pub wall_bounce_b: f32,
    /// Mini-zone wall jump: height gained and push away from the wall.
    pub wall_mini_jump_height: f32,
    pub wall_mini_push: f32,

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
        ground_kick_accel: sheet::GROUND_START_KICK,
        ground_kick_until_frac: sheet::GROUND_KICK_UNTIL_FRAC,
        sprint_ramp_accel: sheet::SPRINT_RAMP_ACCEL,
        friction: sheet::FRICTION,
        stop_speed: sheet::STOP_SPEED,
        crouch_friction_scale: sheet::CROUCH_FRICTION_SCALE,
        crouch_in_time: sheet::CROUCH_IN_TIME,
        crouch_out_time: sheet::CROUCH_OUT_TIME,
        slide_boost: sheet::SLIDE_BOOST,
        slide_boost_cap: sheet::SLIDE_BOOST_CAP,
        slide_boost_cooldown: sheet::SLIDE_BOOST_COOLDOWN,
        slide_decel_const: sheet::SLIDE_DECEL_CONST,
        slide_decel_linear: sheet::SLIDE_DECEL_LINEAR,
        slide_end_speed: sheet::SLIDE_END_SPEED,
        slide_start_min_speed: sheet::SLIDE_START_MIN_SPEED,
        lurch_strength_start: sheet::LURCH_STRENGTH_START,
        lurch_strength: sheet::LURCH_STRENGTH,
        lurch_peak_time: sheet::LURCH_PEAK_TIME,
        lurch_window: sheet::LURCH_WINDOW,
        lurch_speed_cutoff: sheet::LURCH_SPEED_CUTOFF,
        jump_fatigue_min_frac: sheet::JUMP_FATIGUE_MIN_FRAC,
        jump_fatigue_start: sheet::JUMP_FATIGUE_START,
        jump_fatigue_end: sheet::JUMP_FATIGUE_END,
        slide_jump_mult: sheet::SLIDE_JUMP_MULT,
        slide_jump_min_speed: sheet::SLIDE_JUMP_MIN_SPEED,
        bhop_skip_time: sheet::BHOP_SKIP_TIME,
        coyote_time: sheet::COYOTE_TIME,
        bhop_speed_penalty: sheet::BHOP_SPEED_PENALTY,
        mantle_duration_base: sheet::MANTLE_DURATION_BASE,
        mantle_duration_per_unit: sheet::MANTLE_DURATION_PER_UNIT,
        mantle_forward: sheet::MANTLE_FORWARD,
        mantle_max_height: sheet::MANTLE_MAX_HEIGHT,
        mantle_ground_max_height: sheet::MANTLE_GROUND_MAX_HEIGHT,
        superglide_window: sheet::SUPERGLIDE_WINDOW,
        superglide_crouch_ticks: sheet::SUPERGLIDE_CROUCH_TICKS,
        mantle_pull_time: sheet::MANTLE_PULL_TIME,
        mantle_grab_min_height: sheet::MANTLE_GRAB_MIN_HEIGHT,
        mantle_boost_mult: sheet::MANTLE_BOOST_MULT,
        mantle_boost_jump_height: sheet::MANTLE_BOOST_JUMP_HEIGHT,
        mantle_boost_enabled: sheet::MANTLE_BOOST_ENABLED,
        climb_attach_vz: sheet::CLIMB_ATTACH_VZ,
        climb_accel: sheet::CLIMB_ACCEL,
        climb_peak_speed: sheet::CLIMB_PEAK_SPEED,
        climb_decay_rate: sheet::CLIMB_DECAY_RATE,
        climb_end_push_up: sheet::CLIMB_END_PUSH_UP,
        climb_end_push_out: sheet::CLIMB_END_PUSH_OUT,
        wall_jump_push: sheet::WALL_JUMP_PUSH,
        wall_probe_dist: sheet::WALL_PROBE_DIST,
        wall_push_min_dot: sheet::WALL_PUSH_MIN_DOT,
        wall_push_min_speed: sheet::WALL_PUSH_MIN_SPEED,
        mantle_side_decay: sheet::MANTLE_SIDE_DECAY,
        climb_side_speed: sheet::CLIMB_SIDE_SPEED,
        climb_up_side_speed: sheet::CLIMB_UP_SIDE_SPEED,
        climb_side_accel: sheet::CLIMB_SIDE_ACCEL,
        lurch_max_delta_frac: sheet::LURCH_MAX_DELTA_FRAC,
        no_mantle_vz_decel: sheet::NO_MANTLE_VZ_DECEL,
        no_mantle_vz_drag: sheet::NO_MANTLE_VZ_DRAG,
        climb_up_side_accel: sheet::CLIMB_UP_SIDE_ACCEL,
        climb_side_vz_drag: sheet::CLIMB_SIDE_VZ_DRAG,
        climb_peak_time: sheet::CLIMB_PEAK_TIME,
        climb_nonupward_time: sheet::CLIMB_NONUPWARD_TIME,
        climb_nonupward_min_vz: sheet::CLIMB_NONUPWARD_MIN_VZ,
        climb_look_min_dot: sheet::CLIMB_LOOK_MIN_DOT,
        climb_suction: sheet::CLIMB_SUCTION,
        mantle_input_min_dot: sheet::MANTLE_INPUT_MIN_DOT,
        climb_space_height: sheet::CLIMB_SPACE_HEIGHT,
        climb_attach_offset: sheet::CLIMB_ATTACH_OFFSET,
        climb_mini_zone_top: sheet::CLIMB_MINI_ZONE_TOP,
        climb_green_zone_top: sheet::CLIMB_GREEN_ZONE_TOP,
        climb_detach_penalty: sheet::CLIMB_DETACH_PENALTY,
        climb_reattach_min_dot: sheet::CLIMB_REATTACH_MIN_DOT,
        wall_bounce_a: sheet::WALL_BOUNCE_A,
        wall_bounce_b: sheet::WALL_BOUNCE_B,
        wall_mini_jump_height: sheet::WALL_MINI_JUMP_HEIGHT,
        wall_mini_push: sheet::WALL_MINI_PUSH,
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

    /// Jump height fraction after `ground_time` seconds on the ground since a
    /// landing that ended a jump (1.0 when not fatigued).
    pub fn jump_fatigue_frac(&self, ground_time: f32) -> f32 {
        let span = (self.jump_fatigue_end - self.jump_fatigue_start).max(1e-6);
        let a = ((ground_time - self.jump_fatigue_start) / span).clamp(0.0, 1.0);
        self.jump_fatigue_min_frac + (1.0 - self.jump_fatigue_min_frac) * a
    }

    /// Lurch strength for a press `t` seconds after leaving the ground.
    pub fn lurch_k(&self, t: f32) -> f32 {
        if t < 0.0 || t >= self.lurch_window {
            0.0
        } else if t <= self.lurch_peak_time {
            let a = t / self.lurch_peak_time.max(1e-6);
            self.lurch_strength_start + (self.lurch_strength - self.lurch_strength_start) * a
        } else {
            let a =
                (t - self.lurch_peak_time) / (self.lurch_window - self.lurch_peak_time).max(1e-6);
            self.lurch_strength * (1.0 - a)
        }
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
