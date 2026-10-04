//! Everything that persists between ticks.

use apex_math::Vec3;

use crate::input::Buttons;

/// High-level movement mode. Phase 1 has only `Ground`/`Air`; slide, mantle,
/// wall-climb and zipline will be added as their own variants so each owns an
/// explicit set of rules instead of being a pile of flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MoveMode {
    #[default]
    Air,
    Ground,
}

impl MoveMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Air => "air",
            Self::Ground => "ground",
        }
    }
}

/// An in-progress mantle: a scripted move from `start` onto the ledge at `end`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Mantle {
    pub start: Vec3,
    pub end: Vec3,
    /// Horizontal unit direction onto the ledge.
    pub dir: Vec3,
    pub t: f32,
    pub duration: f32,
    /// Ticks since a jump was pressed inside the superglide window (None = no
    /// pending late jump). The following tick decides superglide vs mantle boost.
    pub late_jump_ticks: Option<u8>,
    /// Sideways velocity along the wall carried through the mantle, and the
    /// sideways distance covered so far.
    pub side_vel: Vec3,
    pub side_offset: Vec3,
    /// Sprinting into the mantle: it exits at sprint speed (wiki "Mantle
    /// sprint"), also when the input turns sideways for a sideways superglide.
    pub sprinting: bool,
}

/// An in-progress wall climb.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Climb {
    /// Horizontal wall normal (pointing away from the wall).
    pub normal: Vec3,
    pub t: f32,
    /// The climb speed ceiling has peaked and is decaying (since `peak_t`).
    pub peaked: bool,
    pub peak_t: f32,
    /// Feet height where this climb attached (attach offset is measured from it).
    pub attach_z: f32,
}

/// Complete movement state of one player. Plain data: cloning it and stepping
/// both copies with the same inputs must give bit-identical results.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MovementState {
    /// Hull origin = bottom centre of the hull (the feet). Source units, Z-up.
    pub position: Vec3,
    /// u/s.
    pub velocity: Vec3,
    pub mode: MoveMode,
    /// Normal of the surface we stand on (zero in the air).
    pub ground_normal: Vec3,
    pub crouched: bool,
    /// Buttons held on the previous tick (for press/release edges).
    pub prev_buttons: Buttons,
    /// Ticks simulated so far.
    pub tick: u64,
    /// Simulated seconds so far (f64 so long sessions don't lose precision).
    pub time: f64,
    /// Seconds since we last left / touched the ground. Basis for future
    /// timing windows (jump buffering, coyote time, superglide).
    pub time_in_air: f32,
    pub time_on_ground: f32,
    /// Crouch progress 0 (standing) .. 1 (fully crouched).
    pub crouch_frac: f32,
    /// On the ground in a slide.
    pub sliding: bool,
    /// Seconds since the last slide boost (starts "ready").
    pub time_since_slide_boost: f32,
    /// Movement axes on the previous tick (for directional press edges → lurch).
    pub prev_forward: f32,
    pub prev_right: f32,
    /// The current airtime started with a jump.
    pub air_from_jump: bool,
    /// The last landing ended an airtime that started with a jump (jump fatigue applies).
    pub landed_from_jump: bool,
    /// Traversal in progress (at most one is `Some`).
    pub mantle: Option<Mantle>,
    pub climb: Option<Climb>,
    /// Climb space baseline: feet height at the last ground contact, lowered
    /// by detach penalties and by falling below it.
    pub climb_base: f32,
    /// Last climb's attach height and wall normal this airtime (reattach rule).
    pub last_attach_z: Option<f32>,
    pub last_attach_normal: Vec3,
    /// Wall hit by last tick's move (horizontal normal, zero if none) and how
    /// fast the player was moving into it, for next tick's mantle/climb check.
    pub last_wall_normal: Vec3,
    pub last_wall_impact: f32,
    /// Lurch window clock: time since the last jump (ground, wall, mantle
    /// jump-off, superglide) or since leaving the ground; it keeps running
    /// through mantles and climbs.
    pub lurch_t: f32,
    /// A mantle boost turned lurch off until the next ground contact.
    pub lurch_disabled: bool,
    /// Coyote time: seconds since walking/sliding off the ground without a
    /// jump (None = not available), whether we left in a slide, and how long
    /// we had been on the ground (for the bhop penalty).
    pub coyote: Option<Coyote>,
    /// Seconds since the current slide started, and the boost it gave
    /// (for the deadslide rule).
    pub slide_t: f32,
    pub slide_boost_given: f32,
    /// Fall stun after a hard landing: (seconds since landing, strength 0..1).
    pub fall_stun: Option<(f32, f32)>,
    /// Apex "press" sprint: a sprint press holds until forward is released.
    pub sprint_latched: bool,
}

/// Coyote-time bookkeeping (see `MovementState::coyote`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Coyote {
    pub t: f32,
    pub from_slide: bool,
    pub ground_time: f32,
}

impl MovementState {
    pub fn at(position: Vec3) -> Self {
        Self {
            position,
            climb_base: position.z,
            time_since_slide_boost: f32::MAX,
            ..Self::default()
        }
    }

    /// "air", "ground" or "slide".
    pub fn mode_name(&self) -> &'static str {
        if self.mantle.is_some() {
            "mantle"
        } else if self.climb.is_some() {
            "climb"
        } else if self.sliding {
            "slide"
        } else {
            self.mode.name()
        }
    }

    #[inline]
    pub fn on_ground(&self) -> bool {
        self.mode == MoveMode::Ground
    }

    #[inline]
    pub fn horizontal_speed(&self) -> f32 {
        self.velocity.length_2d()
    }
}
