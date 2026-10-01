//! The one funnel: `MovementState + MovementInput + CollisionWorld + dt -> MovementState`.
//!
//! Pipeline (Phase 1, Source lineage — see docs/MOVEMENT-MODEL.md):
//!
//! ```text
//! wish  ->  crouch  ->  categorize  ->  jump?  ->  ground: friction, accel, step-slide, stick
//!                                              \->  air:    ½g, air-accel, slide, ½g
//!       ->  categorize  ->  timers
//! ```

use apex_collision::CollisionWorld;
use apex_math::{Vec3, yaw_basis};

use crate::collide::{slide_move, stay_on_ground, step_slide_move};
use crate::config::MovementConfig;
use crate::input::{Buttons, MovementInput};
use crate::state::{MoveMode, MovementState};

/// Longest step accepted; a hitch longer than this is simulated as this long.
pub const MAX_STEP_SECONDS: f32 = 0.1;
/// Downward probe distance for ground detection.
const GROUND_PROBE: f32 = 2.0;

/// Per-tick diagnostics. Not part of the state: dropping it changes nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StepReport {
    pub dt: f32,
    /// Horizontal unit wish direction (zero if no movement input).
    pub wish_dir: Vec3,
    pub wish_speed: f32,
    /// Speed removed by friction this tick (u/s).
    pub friction_drop: f32,
    /// Speed added by (air-)acceleration this tick (u/s).
    pub accel_added: f32,
    pub jumped: bool,
    pub landed: bool,
    pub hit_wall: bool,
    pub wall_normal: Vec3,
}

/// Advance one player by `dt` seconds.
pub fn step(
    state: &MovementState,
    input: &MovementInput,
    cfg: &MovementConfig,
    world: &impl CollisionWorld,
    dt: f32,
) -> MovementState {
    step_with_report(state, input, cfg, world, dt).0
}

/// [`step`], also returning what happened inside the tick.
pub fn step_with_report(
    state: &MovementState,
    input: &MovementInput,
    cfg: &MovementConfig,
    world: &impl CollisionWorld,
    dt: f32,
) -> (MovementState, StepReport) {
    let dt = if dt.is_finite() {
        dt.clamp(0.0, MAX_STEP_SECONDS)
    } else {
        0.0
    };
    let mut s = *state;
    let mut rep = StepReport {
        dt,
        ..StepReport::default()
    };
    let pressed = input.buttons.pressed_since(s.prev_buttons);
    let was_on_ground = s.on_ground();

    // --- Wish direction: horizontal only, from yaw. ---
    let (fwd, right) = yaw_basis(input.yaw);
    let wish = fwd * input.forward.clamp(-1.0, 1.0) + right * input.right.clamp(-1.0, 1.0);
    let input_mag = wish.length().min(1.0);
    let wish_dir = wish.normalize_or_zero();

    update_crouch(&mut s, input, cfg, world);
    categorize(&mut s, cfg, world);

    let max_speed = if s.crouched {
        cfg.crouch_speed
    } else if input.buttons.contains(Buttons::SPRINT) && input.forward > 0.0 {
        cfg.sprint_speed
    } else {
        cfg.run_speed
    };
    let wish_speed = max_speed * input_mag;
    rep.wish_dir = wish_dir;
    rep.wish_speed = wish_speed;

    // --- Jump: leaving the ground *before* friction is what preserves speed on a bhop. ---
    let wants_jump =
        pressed.contains(Buttons::JUMP) || (cfg.auto_jump && input.buttons.contains(Buttons::JUMP));
    if s.on_ground() && wants_jump {
        s.velocity.z = cfg.jump_speed(cfg.jump_height);
        s.mode = MoveMode::Air;
        s.ground_normal = Vec3::ZERO;
        rep.jumped = true;
    }

    let hull = cfg.hull(s.crouched);
    if s.on_ground() {
        rep.friction_drop = apply_friction(&mut s.velocity, cfg, dt);
        s.velocity.z = 0.0;
        rep.accel_added = accelerate(&mut s.velocity, wish_dir, wish_speed, cfg.ground_accel, dt);
        s.velocity.z = 0.0;
        clamp_velocity(&mut s.velocity, cfg);

        let moved = step_slide_move(world, hull, cfg, s.position, s.velocity, dt);
        s.position = stay_on_ground(world, hull, cfg, moved.position);
        s.velocity = moved.velocity;
        rep.hit_wall = moved.hit_wall;
        rep.wall_normal = moved.wall_normal;
    } else {
        // Split gravity around the move: exact for constant gravity (the
        // trajectory is the midpoint rule), and it is what Source does.
        s.velocity.z -= 0.5 * cfg.gravity * dt;
        rep.accel_added = air_accelerate(&mut s.velocity, wish_dir, wish_speed, cfg, dt);
        clamp_velocity(&mut s.velocity, cfg);

        let moved = slide_move(world, hull, cfg, s.position, s.velocity, dt);
        s.position = moved.position;
        s.velocity = moved.velocity;
        s.velocity.z -= 0.5 * cfg.gravity * dt;
        rep.hit_wall = moved.hit_wall;
        rep.wall_normal = moved.wall_normal;
    }

    categorize(&mut s, cfg, world);
    if s.on_ground() {
        s.velocity.z = 0.0;
    }
    rep.landed = !was_on_ground && !rep.jumped && s.on_ground();

    // --- Timers / bookkeeping. ---
    if s.on_ground() {
        s.time_on_ground = if was_on_ground {
            s.time_on_ground + dt
        } else {
            0.0
        };
        s.time_in_air = 0.0;
    } else {
        s.time_in_air = if was_on_ground {
            0.0
        } else {
            s.time_in_air + dt
        };
        s.time_on_ground = 0.0;
    }
    s.prev_buttons = input.buttons;
    s.tick += 1;
    s.time += f64::from(dt);
    (s, rep)
}

/// Ground friction (Source `Friction`). Returns the speed removed.
pub fn apply_friction(velocity: &mut Vec3, cfg: &MovementConfig, dt: f32) -> f32 {
    let speed = velocity.length();
    if speed < 0.1 {
        *velocity = Vec3::ZERO;
        return speed;
    }
    let control = speed.max(cfg.stop_speed);
    let drop = control * cfg.friction * dt;
    let new_speed = (speed - drop).max(0.0);
    *velocity *= new_speed / speed;
    speed - new_speed
}

/// Quake/Source acceleration: add speed along `wish_dir` until the *projection*
/// of velocity onto it reaches `wish_speed`. Returns the speed added.
pub fn accelerate(
    velocity: &mut Vec3,
    wish_dir: Vec3,
    wish_speed: f32,
    accel: f32,
    dt: f32,
) -> f32 {
    let current = velocity.dot(wish_dir);
    let add = wish_speed - current;
    if add <= 0.0 {
        return 0.0;
    }
    let amount = (accel * dt * wish_speed).min(add);
    *velocity = velocity.mul_add(amount, wish_dir);
    amount
}

/// Source air acceleration: the projection test uses a capped wish speed while
/// the step size uses the full one. This asymmetry is what makes air strafing
/// gain speed. Apex's real air model is expected to differ (Phase 3).
pub fn air_accelerate(
    velocity: &mut Vec3,
    wish_dir: Vec3,
    wish_speed: f32,
    cfg: &MovementConfig,
    dt: f32,
) -> f32 {
    let capped = wish_speed.min(cfg.air_speed_cap);
    let current = velocity.dot(wish_dir);
    let add = capped - current;
    if add <= 0.0 {
        return 0.0;
    }
    let amount = (cfg.air_accel * wish_speed * dt).min(add);
    *velocity = velocity.mul_add(amount, wish_dir);
    amount
}

fn clamp_velocity(v: &mut Vec3, cfg: &MovementConfig) {
    let m = cfg.max_velocity;
    v.x = v.x.clamp(-m, m);
    v.y = v.y.clamp(-m, m);
    v.z = v.z.clamp(-m, m);
}

/// Decide Ground vs Air by probing just below the feet (Source `CategorizePosition`).
fn categorize(s: &mut MovementState, cfg: &MovementConfig, world: &impl CollisionWorld) {
    if s.velocity.z > cfg.non_jump_velocity {
        s.mode = MoveMode::Air;
        s.ground_normal = Vec3::ZERO;
        return;
    }
    let hull = cfg.hull(s.crouched);
    let below = s.position - Vec3::new(0.0, 0.0, GROUND_PROBE);
    let tr = world.trace_hull(s.position, below, hull);
    if tr.fraction < 1.0 && !tr.all_solid && tr.normal.z >= cfg.walkable_normal_z {
        s.mode = MoveMode::Ground;
        s.ground_normal = tr.normal;
        if !tr.start_solid {
            s.position = tr.end_pos;
        }
    } else {
        s.mode = MoveMode::Air;
        s.ground_normal = Vec3::ZERO;
    }
}

/// Instant crouch / uncrouch (Apex has a transition time; Phase 3).
/// In the air, crouching pulls the feet up so the head stays put.
fn update_crouch(
    s: &mut MovementState,
    input: &MovementInput,
    cfg: &MovementConfig,
    world: &impl CollisionWorld,
) {
    let want = input.buttons.contains(Buttons::CROUCH);
    let diff = cfg.stand_hull.maxs.z - cfg.crouch_hull.maxs.z;
    if want && !s.crouched {
        if !s.on_ground() {
            let raised = s.position + Vec3::new(0.0, 0.0, diff);
            if world.hull_fits(raised, cfg.crouch_hull) {
                s.position = raised;
            }
        }
        s.crouched = true;
    } else if !want && s.crouched {
        let target = if s.on_ground() {
            s.position
        } else {
            s.position - Vec3::new(0.0, 0.0, diff)
        };
        if world.hull_fits(target, cfg.stand_hull) {
            s.position = target;
            s.crouched = false;
        } else if !s.on_ground() && world.hull_fits(s.position, cfg.stand_hull) {
            s.crouched = false;
        }
    }
}
