//! Movement telemetry: per-tick trace rows, sparse input scripts, and a
//! deterministic replay runner.
//!
//! The point of this crate is comparison. A trace recorded from Apex/R5 (by
//! hand or by tooling) and a trace produced here from the same input script
//! must line up column for column, so both use [`TraceRow`].

pub mod flowstate;

use std::io::{self, Write};

use apex_collision::CollisionWorld;
use apex_core::{
    Buttons, MovementConfig, MovementInput, MovementState, StepReport, Vec3, step_with_report,
};
use serde::{Deserialize, Serialize};

/// One simulated tick, flattened for CSV/JSON.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceRow {
    pub tick: u64,
    pub time: f64,
    pub dt: f32,
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
    pub vel_x: f32,
    pub vel_y: f32,
    pub vel_z: f32,
    pub speed_h: f32,
    pub mode: String,
    pub crouched: bool,
    pub in_forward: f32,
    pub in_right: f32,
    pub in_buttons: u32,
    pub yaw: f32,
    pub pitch: f32,
    pub wish_speed: f32,
    pub accel_added: f32,
    pub friction_drop: f32,
    pub jumped: bool,
    pub landed: bool,
    pub hit_wall: bool,
    pub lurch_k: f32,
    pub time_in_air: f32,
}

impl TraceRow {
    /// Row describing the state *after* a step taken with `input`.
    pub fn new(s: &MovementState, input: &MovementInput, rep: &StepReport) -> Self {
        Self {
            tick: s.tick,
            time: s.time,
            dt: rep.dt,
            pos_x: s.position.x,
            pos_y: s.position.y,
            pos_z: s.position.z,
            vel_x: s.velocity.x,
            vel_y: s.velocity.y,
            vel_z: s.velocity.z,
            speed_h: s.horizontal_speed(),
            mode: s.mode_name().to_owned(),
            crouched: s.crouched,
            in_forward: input.forward,
            in_right: input.right,
            in_buttons: input.buttons.0,
            yaw: input.yaw,
            pitch: input.pitch,
            wish_speed: rep.wish_speed,
            accel_added: rep.accel_added,
            friction_drop: rep.friction_drop,
            jumped: rep.jumped,
            landed: rep.landed,
            hit_wall: rep.hit_wall,
            lurch_k: rep.lurch_k,
            time_in_air: s.time_in_air,
        }
    }
}

pub const CSV_HEADER: &str = "tick,time,dt,pos_x,pos_y,pos_z,vel_x,vel_y,vel_z,speed_h,mode,crouched,in_forward,in_right,in_buttons,yaw,pitch,wish_speed,accel_added,friction_drop,jumped,landed,hit_wall,lurch_k,time_in_air";

/// Write rows as CSV. Floats use Rust's shortest round-trip formatting, so the
/// CSV reproduces the exact f32 bits when parsed back.
pub fn write_csv(rows: &[TraceRow], mut out: impl Write) -> io::Result<()> {
    writeln!(out, "{CSV_HEADER}")?;
    for r in rows {
        writeln!(
            out,
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            r.tick,
            r.time,
            r.dt,
            r.pos_x,
            r.pos_y,
            r.pos_z,
            r.vel_x,
            r.vel_y,
            r.vel_z,
            r.speed_h,
            r.mode,
            r.crouched as u8,
            r.in_forward,
            r.in_right,
            r.in_buttons,
            r.yaw,
            r.pitch,
            r.wish_speed,
            r.accel_added,
            r.friction_drop,
            r.jumped as u8,
            r.landed as u8,
            r.hit_wall as u8,
            r.lurch_k,
            r.time_in_air
        )?;
    }
    Ok(())
}

/// Starting conditions for a script.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptStart {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
}

/// A change to the held input, applied at the start of `tick` and kept until
/// changed again. Anything omitted stays as it was.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InputEvent {
    pub tick: u64,
    pub forward: Option<f32>,
    pub right: Option<f32>,
    /// Absolute yaw, degrees.
    pub yaw: Option<f32>,
    /// Yaw change per tick from this tick on (mouse sweep), degrees/tick.
    pub yaw_rate: Option<f32>,
    pub pitch: Option<f32>,
    /// Buttons to hold down: "jump", "crouch", "sprint".
    pub press: Vec<String>,
    pub release: Vec<String>,
    /// Override the step length from this tick on (seconds). Lets a script
    /// reproduce Apex's variable per-usercmd frame time.
    pub dt: Option<f32>,
}

/// A deterministic input sequence. See `scripts/inputs/*.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputScript {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Default ticks per second (dt = 1 / tick_rate).
    pub tick_rate: f64,
    /// Number of ticks to simulate.
    pub ticks: u64,
    #[serde(default)]
    pub start: ScriptStart,
    #[serde(default)]
    pub events: Vec<InputEvent>,
}

#[derive(Debug)]
pub enum ScriptError {
    UnknownButton(String),
    BadTickRate(f64),
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownButton(b) => {
                write!(f, "unknown button {b:?} (expected jump, crouch or sprint)")
            }
            Self::BadTickRate(r) => write!(f, "tick_rate must be > 0, got {r}"),
        }
    }
}

impl std::error::Error for ScriptError {}

pub fn button_from_name(name: &str) -> Result<Buttons, ScriptError> {
    match name.to_ascii_lowercase().as_str() {
        "jump" => Ok(Buttons::JUMP),
        "crouch" | "duck" => Ok(Buttons::CROUCH),
        "sprint" => Ok(Buttons::SPRINT),
        _ => Err(ScriptError::UnknownButton(name.to_owned())),
    }
}

impl InputScript {
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    pub fn start_state(&self) -> MovementState {
        let mut s = MovementState::at(Vec3::from_array(self.start.position));
        s.velocity = Vec3::from_array(self.start.velocity);
        s
    }

    /// Expand the sparse events into one `(input, dt)` per tick.
    pub fn expand(&self) -> Result<Vec<(MovementInput, f32)>, ScriptError> {
        if self.tick_rate.is_nan() || self.tick_rate <= 0.0 {
            return Err(ScriptError::BadTickRate(self.tick_rate));
        }
        let mut events = self.events.clone();
        events.sort_by_key(|e| e.tick);
        let mut ev = events.iter().peekable();

        let mut held = MovementInput {
            yaw: self.start.yaw,
            pitch: self.start.pitch,
            ..MovementInput::default()
        };
        let mut yaw_rate = 0.0_f32;
        let mut dt = (1.0 / self.tick_rate) as f32;
        let mut out = Vec::with_capacity(self.ticks as usize);

        for tick in 0..self.ticks {
            if tick > 0 {
                held.yaw = apex_math::wrap_degrees(held.yaw + yaw_rate);
            }
            while let Some(e) = ev.next_if(|e| e.tick <= tick) {
                if let Some(v) = e.forward {
                    held.forward = v;
                }
                if let Some(v) = e.right {
                    held.right = v;
                }
                if let Some(v) = e.yaw {
                    held.yaw = v;
                }
                if let Some(v) = e.yaw_rate {
                    yaw_rate = v;
                }
                if let Some(v) = e.pitch {
                    held.pitch = v;
                }
                if let Some(v) = e.dt {
                    dt = v;
                }
                for b in &e.press {
                    held.buttons.set(button_from_name(b)?, true);
                }
                for b in &e.release {
                    held.buttons.set(button_from_name(b)?, false);
                }
            }
            out.push((held, dt));
        }
        Ok(out)
    }

    /// Run the script and return one row per tick.
    pub fn run(
        &self,
        world: &impl CollisionWorld,
        cfg: &MovementConfig,
    ) -> Result<Vec<TraceRow>, ScriptError> {
        let mut s = self.start_state();
        let mut rows = Vec::with_capacity(self.ticks as usize);
        for (input, dt) in self.expand()? {
            let (next, rep) = step_with_report(&s, &input, cfg, world, dt);
            rows.push(TraceRow::new(&next, &input, &rep));
            s = next;
        }
        Ok(rows)
    }
}

/// First tick at which two traces disagree beyond `tol` in position or
/// velocity, with the largest component difference. `None` = they match.
pub fn first_divergence(a: &[TraceRow], b: &[TraceRow], tol: f32) -> Option<(u64, f32)> {
    for (ra, rb) in a.iter().zip(b) {
        let diffs = [
            ra.pos_x - rb.pos_x,
            ra.pos_y - rb.pos_y,
            ra.pos_z - rb.pos_z,
            ra.vel_x - rb.vel_x,
            ra.vel_y - rb.vel_y,
            ra.vel_z - rb.vel_z,
        ];
        let worst = diffs.iter().fold(0.0_f32, |m, d| m.max(d.abs()));
        if worst > tol {
            return Some((ra.tick, worst));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use apex_collision::maps::proving_ground;

    const SCRIPT: &str = r#"{
        "name": "jump-test", "tick_rate": 60, "ticks": 120,
        "events": [
            { "tick": 0,  "forward": 1, "press": ["sprint"] },
            { "tick": 20, "press": ["jump"] },
            { "tick": 21, "release": ["jump"] },
            { "tick": 60, "yaw_rate": 1.5 }
        ]
    }"#;

    #[test]
    fn script_expands_held_state() {
        let s = InputScript::from_json(SCRIPT).unwrap();
        let ticks = s.expand().unwrap();
        assert_eq!(ticks.len(), 120);
        assert!(ticks[19].0.buttons.contains(Buttons::SPRINT));
        assert!(!ticks[19].0.buttons.contains(Buttons::JUMP));
        assert!(ticks[20].0.buttons.contains(Buttons::JUMP));
        assert!(!ticks[21].0.buttons.contains(Buttons::JUMP));
        assert_eq!(ticks[61].0.yaw, 1.5);
    }

    #[test]
    fn replay_is_deterministic_and_round_trips_through_csv() {
        let s = InputScript::from_json(SCRIPT).unwrap();
        let w = proving_ground();
        let cfg = MovementConfig::default();
        let a = s.run(&w, &cfg).unwrap();
        let b = s.run(&w, &cfg).unwrap();
        assert_eq!(first_divergence(&a, &b, 0.0), None);
        assert!(a.iter().any(|r| r.jumped));

        let mut csv = Vec::new();
        write_csv(&a, &mut csv).unwrap();
        let text = String::from_utf8(csv).unwrap();
        let last = text.lines().last().unwrap();
        let pos_x: f32 = last.split(',').nth(3).unwrap().parse().unwrap();
        assert_eq!(pos_x.to_bits(), a.last().unwrap().pos_x.to_bits());
    }

    #[test]
    fn unknown_button_is_an_error() {
        let mut s = InputScript::from_json(SCRIPT).unwrap();
        s.events[0].press = vec!["fly".into()];
        assert!(s.expand().is_err());
    }
}
