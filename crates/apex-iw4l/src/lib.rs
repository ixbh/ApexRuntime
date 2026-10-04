//! apex-iw4l — runs `apex-core` movement inside IW4L (the open-source MW2
//! runtime, <https://github.com/vladtrc/iw4L>, Apache-2.0).
//!
//! IW4L calls `movement_iw4::pmove(ps, cmd, context, collision, …)` once per
//! player per usercmd from `sim::step` (server authority, client prediction
//! and replay all share that call). This crate is everything needed to call
//! `apex_core::step` there instead, without IW4L changes of its own:
//!
//! * [`Iw4Collision`] — `apex_collision::CollisionWorld` over any IW4L
//!   `CollisionBackend` (its clipmap / brush / model traces);
//! * [`input_from_cmd`] / [`cmd_dt`] — usercmd + view angles -> `MovementInput`
//!   and the command's frame time (Apex movement is per-usercmd, like IW4);
//! * [`config_for_iw4`] — IW4 hull sizes so map clearances fit;
//! * [`sync_to_playerstate`] / [`adopt_from_playerstate`] — mirror the result
//!   into `PlayerState` (rendering, animation, networking) and pick up
//!   host-side teleports (spawns, scripts).
//!
//! Units match (inches, Z-up). See docs/IW4L-ADAPTER.md for the plan.

use apex_collision::{CollisionWorld, Trace};
use apex_core::{Buttons, MovementConfig, MovementInput, MovementState};
use apex_math::{Aabb, Vec3};
use movement_iw4::{CollisionBackend, GroundTraceInput};
use playerstate_iw4::{ENTITYNUM_NONE, PlayerState, UserCmd, buttons, pm_flags};

/// IW4L's player movement content mask (what `sim::step` passes for pmove).
pub const PLAYER_TRACEMASK: u32 = 0x0281_0011;
/// Ground entity number for "standing on the world".
pub const ENTITYNUM_WORLD: i32 = 0x7FE;
/// IW4 player hull: 30 wide, 70 tall standing, 50 crouched.
pub const IW4_HALF_WIDTH: f32 = 15.0;
pub const IW4_STAND_HEIGHT: f32 = 70.0;
pub const IW4_CROUCH_HEIGHT: f32 = 50.0;
/// IW4 view heights above the origin.
pub const IW4_STAND_EYE: f32 = 60.0;
pub const IW4_CROUCH_EYE: f32 = 40.0;

/// `CollisionWorld` over an IW4L collision backend.
pub struct Iw4Collision<'a, C> {
    pub backend: &'a C,
    pub tracemask: u32,
}

impl<'a, C: CollisionBackend> Iw4Collision<'a, C> {
    pub fn new(backend: &'a C) -> Self {
        Self {
            backend,
            tracemask: PLAYER_TRACEMASK,
        }
    }
}

fn v(a: [f32; 3]) -> Vec3 {
    Vec3::new(a[0], a[1], a[2])
}

impl<C: CollisionBackend> CollisionWorld for Iw4Collision<'_, C> {
    fn trace_hull(&self, start: Vec3, end: Vec3, hull: Aabb) -> Trace {
        let t = self.backend.trace(GroundTraceInput {
            start: start.to_array(),
            end: end.to_array(),
            mins: hull.mins.to_array(),
            maxs: hull.maxs.to_array(),
            tracemask: self.tracemask,
        });
        let fraction = t.fraction.clamp(0.0, 1.0);
        Trace {
            fraction,
            // Recomputed from the fraction: not every IW4L backend fills endpos.
            end_pos: start + (end - start) * fraction,
            normal: if fraction < 1.0 {
                v(t.normal)
            } else {
                Vec3::ZERO
            },
            start_solid: t.startsolid != 0,
            all_solid: t.allsolid != 0,
            surface: t.surface_flags,
        }
    }
}

/// The command's frame time in seconds (IW4 pmove clamps msec to 1..200).
pub fn cmd_dt(ps: &PlayerState, cmd: &UserCmd) -> f32 {
    let msec = cmd.server_time.wrapping_sub(ps.command_time).clamp(1, 200);
    msec as f32 * 0.001
}

/// usercmd + the player's view angles (degrees, `[pitch, yaw, roll]` as in
/// `PlayerState::viewangles`) -> apex movement input.
pub fn input_from_cmd(cmd: &UserCmd, viewangles: [f32; 3]) -> MovementInput {
    let axis = |m: i8| (f32::from(m) / 127.0).clamp(-1.0, 1.0);
    let mut b = Buttons::NONE;
    b.set(Buttons::JUMP, cmd.buttons & buttons::JUMP != 0);
    b.set(Buttons::CROUCH, cmd.buttons & buttons::CROUCH != 0);
    b.set(Buttons::SPRINT, cmd.buttons & buttons::SPRINT != 0);
    MovementInput {
        forward: axis(cmd.forwardmove),
        right: axis(cmd.rightmove),
        buttons: b,
        yaw: viewangles[1],
        pitch: viewangles[0],
    }
}

/// Apex rules with IW4's hull sizes and view heights, so map doorways,
/// vents and ledge heights work as the maps were built.
pub fn config_for_iw4(mut cfg: MovementConfig) -> MovementConfig {
    let w = IW4_HALF_WIDTH;
    cfg.stand_hull = Aabb::new(Vec3::new(-w, -w, 0.0), Vec3::new(w, w, IW4_STAND_HEIGHT));
    cfg.crouch_hull = Aabb::new(Vec3::new(-w, -w, 0.0), Vec3::new(w, w, IW4_CROUCH_HEIGHT));
    cfg.stand_eye_height = IW4_STAND_EYE;
    cfg.crouch_eye_height = IW4_CROUCH_EYE;
    cfg
}

/// Mirror the apex state into IW4's `PlayerState` after a step: position,
/// velocity, ground, stance and mantle flags, view height.
pub fn sync_to_playerstate(ms: &MovementState, cfg: &MovementConfig, ps: &mut PlayerState) {
    ps.origin = ms.position.to_array();
    ps.velocity = ms.velocity.to_array();
    ps.ground_entity_num = if ms.on_ground() {
        ENTITYNUM_WORLD
    } else {
        ENTITYNUM_NONE
    };
    let set = |flags: &mut u32, bit: u32, on: bool| {
        if on {
            *flags |= bit;
        } else {
            *flags &= !bit;
        }
    };
    set(&mut ps.pm_flags, pm_flags::CROUCH, ms.crouched);
    set(&mut ps.pm_flags, pm_flags::PRONE, false);
    set(&mut ps.pm_flags, pm_flags::MANTLE, ms.mantle.is_some());
    let eye =
        cfg.stand_eye_height + (cfg.crouch_eye_height - cfg.stand_eye_height) * ms.crouch_frac;
    ps.view_height_target = eye.round() as i32;
    ps.view_height_current = eye;
}

/// Pick up host-side changes before a step: if IW4 moved the player itself
/// (spawn, script teleport, knockback), adopt its origin/velocity.
pub fn adopt_from_playerstate(ps: &PlayerState, ms: &mut MovementState) {
    let pos = v(ps.origin);
    let vel = v(ps.velocity);
    if (pos - ms.position).length() > 0.01 {
        *ms = MovementState {
            position: pos,
            velocity: vel,
            climb_base: pos.z,
            ..*ms
        };
        ms.mantle = None;
        ms.climb = None;
    } else if (vel - ms.velocity).length() > 0.01 {
        ms.velocity = vel;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apex_core::{settle, step_with_report};
    use movement_iw4::GroundTraceInput;
    use trace_iw4::Trace as Iw4Trace;

    /// Stand-in IW4L backend: solid half-space z < 0 (a flat floor).
    struct Floor;

    impl CollisionBackend for Floor {
        fn trace(&self, input: GroundTraceInput) -> Iw4Trace {
            let b0 = input.start[2] + input.mins[2];
            let b1 = input.end[2] + input.mins[2];
            let mut t = Iw4Trace {
                fraction: 1.0,
                endpos: input.end,
                ..Iw4Trace::default()
            };
            if b0 < 0.0 {
                t.startsolid = 1;
                t.allsolid = u8::from(b1 < 0.0);
                t.fraction = 0.0;
                t.endpos = input.start;
            } else if b1 < 0.0 {
                t.fraction = ((b0 - 0.03125) / (b0 - b1)).max(0.0);
                t.normal = [0.0, 0.0, 1.0];
            }
            t
        }
    }

    fn cmd(time: i32, fwd: i8, right: i8, btn: u32) -> UserCmd {
        UserCmd {
            server_time: time,
            forwardmove: fwd,
            rightmove: right,
            buttons: btn,
            ..UserCmd::default()
        }
    }

    #[test]
    fn input_mapping() {
        let i = input_from_cmd(
            &cmd(0, 127, -127, buttons::JUMP | buttons::SPRINT),
            [5.0, 90.0, 0.0],
        );
        assert_eq!((i.forward, i.right, i.yaw, i.pitch), (1.0, -1.0, 90.0, 5.0));
        assert!(i.buttons.contains(Buttons::JUMP) && i.buttons.contains(Buttons::SPRINT));
        assert!(!i.buttons.contains(Buttons::CROUCH));
    }

    #[test]
    fn apex_movement_runs_through_an_iw4_backend() {
        let backend = Floor;
        let world = Iw4Collision::new(&backend);
        let cfg = config_for_iw4(MovementConfig::default());
        let mut ms = MovementState::at(Vec3::new(0.0, 0.0, 1.0));
        settle(&mut ms, &cfg, &world);
        assert!(
            ms.on_ground() && ms.position.z.abs() < 0.1,
            "{:?}",
            ms.position
        );

        // Sprint forward for 1.5 s of 7 ms usercmds (~144 fps), like IW4L does.
        let mut ps = PlayerState::ZERO;
        let mut time = 0;
        for _ in 0..214 {
            time += 7;
            let c = cmd(time, 127, 0, buttons::SPRINT);
            let dt = cmd_dt(&ps, &c);
            ps.command_time = c.server_time;
            ms = step_with_report(&ms, &input_from_cmd(&c, [0.0, 0.0, 0.0]), &cfg, &world, dt).0;
            sync_to_playerstate(&ms, &cfg, &mut ps);
        }
        assert!(
            (ms.horizontal_speed() - 299.0).abs() < 1.0,
            "sprint {}",
            ms.horizontal_speed()
        );
        assert_eq!(ps.ground_entity_num, ENTITYNUM_WORLD);
        assert_eq!(ps.origin, ms.position.to_array());

        // Jump: airborne, playerState says so.
        time += 7;
        let c = cmd(time, 127, 0, buttons::SPRINT | buttons::JUMP);
        let dt = cmd_dt(&ps, &c);
        ms = step_with_report(&ms, &input_from_cmd(&c, [0.0, 0.0, 0.0]), &cfg, &world, dt).0;
        sync_to_playerstate(&ms, &cfg, &mut ps);
        assert!(!ms.on_ground() && ms.velocity.z > 250.0);
        assert_eq!(ps.ground_entity_num, ENTITYNUM_NONE);
    }

    #[test]
    fn host_teleports_are_adopted() {
        let mut ms = MovementState::at(Vec3::new(0.0, 0.0, 0.0));
        let ps = PlayerState {
            origin: [500.0, 20.0, 64.0],
            ..PlayerState::ZERO
        };
        adopt_from_playerstate(&ps, &mut ms);
        assert_eq!(ms.position, Vec3::new(500.0, 20.0, 64.0));
        assert_eq!(ms.climb_base, 64.0);
    }
}
