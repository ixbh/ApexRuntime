//! The one funnel: `MovementState + MovementInput + CollisionWorld + dt -> MovementState`.
//!
//! Pipeline (Source lineage, Apex rules from R5Flowstate S21 measurements:
//! docs/MOVEMENT-MODEL.md, docs/MOVEMENT-MEASUREMENTS.md):
//!
//! ```text
//! wish -> crouch (timed) -> categorize -> jump? -> slide state
//!   ground, sliding: slide decel, step-slide, stick
//!   ground, walking: friction, banded accel (kick / run / sprint ramp), step-slide, stick
//!   air:             half g, lurch on new press, air-accel, slide-move, half g
//! -> categorize -> timers
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
    /// Lurch strength applied this tick (0 = no lurch).
    pub lurch_k: f32,
    /// A slide started this tick (`slide_boosted` if it got the boost).
    pub slide_started: bool,
    pub slide_boosted: bool,
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

    update_crouch(&mut s, input, cfg, world, dt);
    categorize(&mut s, cfg, world);

    let base_speed = if input.buttons.contains(Buttons::SPRINT) && input.forward > 0.0 {
        cfg.sprint_speed
    } else {
        cfg.run_speed
    };
    let max_speed = base_speed + (cfg.crouch_speed - base_speed) * s.crouch_frac;
    let wish_speed = max_speed * input_mag;
    rep.wish_dir = wish_dir;
    rep.wish_speed = wish_speed;

    // --- Jump: leaving the ground *before* friction is what preserves speed on a bhop. ---
    let wants_jump =
        pressed.contains(Buttons::JUMP) || (cfg.auto_jump && input.buttons.contains(Buttons::JUMP));
    if s.on_ground() && wants_jump {
        // Apex removes an extra half tick of gravity on the jump tick: with the
        // air move's split gravity below, the position advances by
        // (V - g*dt)*dt and the tick ends at V - 1.5*g*dt. Measured on the S21
        // client at 60 and 144 fps (rise 53.5 vs 55.0); higher fps jumps higher.
        s.velocity.z = cfg.jump_speed(cfg.jump_height) - 0.5 * cfg.gravity * dt;
        s.mode = MoveMode::Air;
        s.ground_normal = Vec3::ZERO;
        s.sliding = false;
        rep.jumped = true;
    }

    update_slide(&mut s, &mut rep, input, pressed, was_on_ground, cfg);

    let hull = cfg.hull(s.crouched);
    if s.on_ground() && s.sliding {
        s.velocity.z = 0.0;
        let speed = s.velocity.length();
        if speed > 0.0 {
            let decel = (cfg.slide_decel_const + cfg.slide_decel_linear * speed) * dt;
            let new_speed = (speed - decel).max(0.0);
            s.velocity *= new_speed / speed;
            rep.friction_drop = speed - new_speed;
        }
        clamp_velocity(&mut s.velocity, cfg);

        let moved = step_slide_move(world, hull, cfg, s.position, s.velocity, dt);
        s.position = stay_on_ground(world, hull, cfg, moved.position);
        s.velocity = moved.velocity;
        rep.hit_wall = moved.hit_wall;
        rep.wall_normal = moved.wall_normal;
    } else if s.on_ground() {
        let speed_before = s.velocity.horizontal().length();
        let friction_dt = if s.crouched {
            dt * cfg.crouch_friction_scale
        } else {
            dt
        };
        rep.friction_drop = apply_friction(&mut s.velocity, cfg, friction_dt);
        s.velocity.z = 0.0;
        // Apex's measured rates are *net* of friction, so the friction removed
        // this tick is added back on top of the banded net rate.
        let amount = ground_accel_rate(speed_before, cfg) * dt + rep.friction_drop;
        rep.accel_added = accelerate_by(&mut s.velocity, wish_dir, wish_speed, amount);
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
        // Lurch time = time since the first airborne tick (the jump tick counts as 0).
        let t_air = if was_on_ground { 0.0 } else { s.time_in_air };
        rep.lurch_k = lurch(&mut s, input, wish_dir, t_air, cfg);
        // A lurch tick replaces the normal air acceleration (fits the S21
        // traces at 60 and 144 fps; adding both overshoots late presses).
        if rep.lurch_k == 0.0 {
            rep.accel_added = air_accelerate(&mut s.velocity, wish_dir, wish_speed, cfg, dt);
        }
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
    s.time_since_slide_boost = (s.time_since_slide_boost + dt).min(f32::MAX / 2.0);
    s.prev_buttons = input.buttons;
    s.prev_forward = input.forward;
    s.prev_right = input.right;
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

/// Like [`accelerate`] but with an explicit speed amount instead of a rate.
pub fn accelerate_by(velocity: &mut Vec3, wish_dir: Vec3, wish_speed: f32, amount: f32) -> f32 {
    let current = velocity.dot(wish_dir);
    let add = wish_speed - current;
    if add <= 0.0 || amount <= 0.0 {
        return 0.0;
    }
    let amount = amount.min(add);
    *velocity = velocity.mul_add(amount, wish_dir);
    amount
}

/// Net ground acceleration (u/s^2) by speed band, measured on the S21 client
/// and identical at 60 and 144 fps: a hard start kick, the run phase, and a
/// slow ramp from run to sprint speed.
pub fn ground_accel_rate(speed: f32, cfg: &MovementConfig) -> f32 {
    if speed < cfg.ground_kick_until_frac * cfg.run_speed {
        cfg.ground_kick_accel
    } else if speed < cfg.run_speed {
        cfg.ground_accel
    } else {
        cfg.sprint_ramp_accel
    }
}

/// Apex lurch: a *new* directional press while airborne turns the horizontal
/// velocity toward the wish direction, keeping its speed. Strength depends on
/// time since takeoff ([`MovementConfig::lurch_k`]). Keys already held at
/// takeoff do nothing, so tap-strafing is many presses, each one lurching.
/// Returns the strength used (0 if none).
fn lurch(
    s: &mut MovementState,
    input: &MovementInput,
    wish_dir: Vec3,
    t_air: f32,
    cfg: &MovementConfig,
) -> f32 {
    let newly = |cur: f32, prev: f32| cur != 0.0 && (prev == 0.0 || cur.signum() != prev.signum());
    if !(newly(input.forward, s.prev_forward) || newly(input.right, s.prev_right)) {
        return 0.0;
    }
    let k = cfg.lurch_k(t_air);
    let h = s.velocity.horizontal();
    let speed = h.length();
    if k <= 0.0 || speed < 1e-3 || speed > cfg.lurch_speed_cutoff || wish_dir == Vec3::ZERO {
        return 0.0;
    }
    let dir = (h / speed * (1.0 - k) + wish_dir * k).normalize_or_zero();
    if dir == Vec3::ZERO {
        // Exactly opposite at k = 0.5: no defined direction; leave it.
        return 0.0;
    }
    s.velocity.x = dir.x * speed;
    s.velocity.y = dir.y * speed;
    k
}

/// Slide entry/exit (ground only). Entry: crouch held, newly pressed or just
/// landed, at or above `slide_start_min_speed`. Off cooldown, the slide adds
/// `slide_boost` up to `slide_boost_cap` (never reduces speed). Exit: crouch
/// released or speed below `slide_end_speed`.
fn update_slide(
    s: &mut MovementState,
    rep: &mut StepReport,
    input: &MovementInput,
    pressed: Buttons,
    was_on_ground: bool,
    cfg: &MovementConfig,
) {
    if !s.on_ground() {
        s.sliding = false;
        return;
    }
    let held = input.buttons.contains(Buttons::CROUCH);
    let speed = s.velocity.horizontal().length();
    if !s.sliding
        && held
        && (pressed.contains(Buttons::CROUCH) || !was_on_ground)
        && speed >= cfg.slide_start_min_speed
    {
        s.sliding = true;
        s.crouched = true;
        rep.slide_started = true;
        if s.time_since_slide_boost >= cfg.slide_boost_cooldown {
            let boosted = (speed + cfg.slide_boost)
                .min(cfg.slide_boost_cap)
                .max(speed);
            s.velocity.x *= boosted / speed;
            s.velocity.y *= boosted / speed;
            s.time_since_slide_boost = 0.0;
            rep.slide_boosted = true;
        }
    } else if s.sliding && (!held || speed < cfg.slide_end_speed) {
        s.sliding = false;
    }
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

/// Crouch. On the ground it is timed (measured ~0.38 s down, ~0.22 s up):
/// `crouch_frac` blends max speed, the crouched hull applies once fully down,
/// and standing up needs headroom. In the air it is instant and pulls the feet
/// up so the head stays put.
fn update_crouch(
    s: &mut MovementState,
    input: &MovementInput,
    cfg: &MovementConfig,
    world: &impl CollisionWorld,
    dt: f32,
) {
    let want = input.buttons.contains(Buttons::CROUCH);
    let diff = cfg.stand_hull.maxs.z - cfg.crouch_hull.maxs.z;
    if !s.on_ground() {
        if want && !s.crouched {
            let raised = s.position + Vec3::new(0.0, 0.0, diff);
            if world.hull_fits(raised, cfg.crouch_hull) {
                s.position = raised;
            }
            s.crouched = true;
        } else if !want && s.crouched {
            let lowered = s.position - Vec3::new(0.0, 0.0, diff);
            if world.hull_fits(lowered, cfg.stand_hull) {
                s.position = lowered;
                s.crouched = false;
            } else if world.hull_fits(s.position, cfg.stand_hull) {
                s.crouched = false;
            }
        }
        s.crouch_frac = if s.crouched { 1.0 } else { 0.0 };
        return;
    }
    if want {
        s.crouch_frac = (s.crouch_frac + dt / cfg.crouch_in_time.max(1e-6)).min(1.0);
        if s.crouch_frac >= 1.0 {
            s.crouched = true;
        }
    } else {
        if s.crouched {
            if !world.hull_fits(s.position, cfg.stand_hull) {
                return;
            }
            s.crouched = false;
        }
        s.crouch_frac = (s.crouch_frac - dt / cfg.crouch_out_time.max(1e-6)).max(0.0);
    }
}
