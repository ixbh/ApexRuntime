//! Fixed-timestep driver that decouples render rate from simulation rate.
//!
//! The core `step` takes `dt` as an argument (like IW4L's `msec`), because
//! Apex processes each usercmd with the *client's* frame time — which is why
//! some tech (superglide) is frame-rate sensitive. Matching a measured trace
//! means replaying its exact per-command `dt`s. For the sandbox and tests we
//! want the opposite: a fixed tick so render FPS cannot change the result.
//! This clock provides that.

/// Accumulates real frame time and says how many fixed ticks to run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FixedClock {
    /// Seconds per tick.
    pub tick_seconds: f64,
    /// Upper bound on ticks per frame (avoids the "spiral of death" after a hitch).
    pub max_ticks_per_frame: u32,
    accumulator: f64,
}

impl FixedClock {
    pub fn new(tick_rate_hz: f64) -> Self {
        Self {
            tick_seconds: 1.0 / tick_rate_hz,
            max_ticks_per_frame: 16,
            accumulator: 0.0,
        }
    }

    pub fn dt(&self) -> f32 {
        self.tick_seconds as f32
    }

    /// Feed one render frame's duration; returns how many ticks to simulate now.
    pub fn advance(&mut self, frame_seconds: f64) -> u32 {
        self.accumulator += frame_seconds.max(0.0);
        let mut ticks = 0;
        while self.accumulator >= self.tick_seconds && ticks < self.max_ticks_per_frame {
            self.accumulator -= self.tick_seconds;
            ticks += 1;
        }
        if ticks == self.max_ticks_per_frame {
            self.accumulator = self.accumulator.min(self.tick_seconds);
        }
        ticks
    }

    /// How far between the last tick and the next one we are, in `[0, 1)`.
    /// Renderers interpolate the presented state with this.
    pub fn alpha(&self) -> f32 {
        (self.accumulator / self.tick_seconds) as f32
    }
}

impl Default for FixedClock {
    /// 60 Hz. Placeholder; Apex usercmd rate is the client frame rate.
    fn default() -> Self {
        Self::new(60.0)
    }
}
