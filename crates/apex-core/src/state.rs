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
}

impl MovementState {
    pub fn at(position: Vec3) -> Self {
        Self {
            position,
            time_since_slide_boost: f32::MAX,
            ..Self::default()
        }
    }

    /// "air", "ground" or "slide".
    pub fn mode_name(&self) -> &'static str {
        if self.sliding {
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
