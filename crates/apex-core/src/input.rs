//! What the player asked for this tick.

/// Button bitset. Bits are ApexRuntime's own, not Source `IN_*` values; host
/// adapters translate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Buttons(pub u32);

impl Buttons {
    pub const NONE: Self = Self(0);
    pub const JUMP: Self = Self(1 << 0);
    pub const CROUCH: Self = Self(1 << 1);
    pub const SPRINT: Self = Self(1 << 2);
    /// Mantle boost on its own key (only with `MovementConfig::mantle_boost_key`).
    pub const MANTLE_BOOST: Self = Self(1 << 3);

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0 && other.0 != 0
    }

    #[inline]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[inline]
    pub fn set(&mut self, other: Self, on: bool) {
        if on {
            self.0 |= other.0;
        } else {
            self.0 &= !other.0;
        }
    }

    /// Buttons that are down now but were up on the previous tick.
    #[inline]
    pub const fn pressed_since(self, previous: Self) -> Self {
        Self(self.0 & !previous.0)
    }
}

impl core::ops::BitOr for Buttons {
    type Output = Self;
    fn bitor(self, o: Self) -> Self {
        self.union(o)
    }
}

/// One tick of player intent.
///
/// `forward` / `right` are analog axes in `[-1, 1]` (keyboard gives -1/0/1).
/// The view angles are absolute, in degrees (see `apex_math` conventions);
/// they are part of the input because Apex movement — lurch especially — is a
/// function of where you are looking *at the tick the command is processed*.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MovementInput {
    pub forward: f32,
    pub right: f32,
    pub buttons: Buttons,
    pub yaw: f32,
    pub pitch: f32,
}

impl MovementInput {
    pub fn forward() -> Self {
        Self {
            forward: 1.0,
            ..Self::default()
        }
    }

    pub fn with_buttons(mut self, b: Buttons) -> Self {
        self.buttons = b;
        self
    }

    pub fn with_yaw(mut self, yaw: f32) -> Self {
        self.yaw = yaw;
        self
    }
}
