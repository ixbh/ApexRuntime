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
    /// Velocity at the start of the tick (landing speed for fall stun).
    pub entry_velocity: Vec3,
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
    /// Traversal events this tick.
    pub mantle_started: bool,
    pub mantle_finished: bool,
    pub superglide: bool,
    pub mantle_boost: bool,
    pub climb_started: bool,
    pub climb_finished: bool,
    pub wall_jump: bool,
    /// The player started the tick inside solid and was pushed out.
    pub unstuck: bool,
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
    // Apex sprint semantics: auto-sprint, or a sprint press that holds until
    // forward is released (the wiki's "Press" sprint) — not hold-to-sprint.
    let forward = input.forward > 0.0;
    let held = input.buttons.contains(Buttons::SPRINT);
    s.sprint_latched = forward && (s.sprint_latched || held);
    let mut sprint_input = *input;
    sprint_input.buttons.set(
        Buttons::SPRINT,
        forward && (cfg.auto_sprint || s.sprint_latched),
    );
    let input = &sprint_input;
    let mut rep = StepReport {
        dt,
        entry_velocity: state.velocity,
        ..StepReport::default()
    };
    let pressed = input.buttons.pressed_since(s.prev_buttons);
    let was_on_ground = s.on_ground();

    // --- Wish direction: horizontal only, from yaw. ---
    let (fwd, right) = yaw_basis(input.yaw);
    let wish = fwd * input.forward.clamp(-1.0, 1.0) + right * input.right.clamp(-1.0, 1.0);
    let input_mag = wish.length().min(1.0);
    let wish_dir = wish.normalize_or_zero();

    let traversing = s.mantle.is_some() || s.climb.is_some();
    if !traversing {
        rep.unstuck = unstick(&mut s, cfg, world);
        update_crouch(&mut s, input, cfg, world, dt);
        categorize(&mut s, cfg, world);
    }

    // A fall stun holds you at walk speed until it has mostly worn off
    // (S21: walk base until ~0.85 s of a full stun, then sprint).
    let stun = fall_stun_factor(s.fall_stun, cfg);
    let base_speed = if input.buttons.contains(Buttons::SPRINT)
        && input.forward > 0.0
        && stun >= cfg.fall_stun_sprint_factor
    {
        cfg.sprint_speed
    } else {
        cfg.run_speed
    };
    let max_speed = (base_speed + (cfg.crouch_speed - base_speed) * s.crouch_frac)
        * fall_stun_factor(s.fall_stun, cfg);
    let wish_speed = max_speed * input_mag;
    rep.wish_dir = wish_dir;
    rep.wish_speed = wish_speed;

    if traversing {
        if s.mantle.is_some() {
            crate::traverse::mantle_tick(
                &mut s, &mut rep, input, pressed, wish_dir, cfg, world, dt,
            );
        } else {
            crate::traverse::climb_tick(&mut s, &mut rep, input, pressed, wish_dir, cfg, world, dt);
        }
        finish_tick(&mut s, &mut rep, input, was_on_ground, cfg, world, dt);
        return (s, rep);
    }

    // Slide state first: landing with crouch held re-enters the slide on the
    // landing tick, so a jump on that tick is already a slide-jump (S21 traces).
    update_slide(&mut s, &mut rep, input, pressed, was_on_ground, cfg, dt);

    // --- Jump: leaving the ground *before* friction is what preserves speed on a bhop. ---
    let wants_jump =
        pressed.contains(Buttons::JUMP) || (cfg.auto_jump && input.buttons.contains(Buttons::JUMP));
    // Coyote time (wiki; S21 traces): shortly after walking or sliding off an
    // edge a jump still works. It adds a full, never-fatigued jump to the
    // current vertical speed (falling at -25 -> +258; -131 -> +140).
    let coyote = if s.on_ground() || s.mantle.is_some() || s.climb.is_some() {
        None
    } else {
        s.coyote.filter(|c| c.t < cfg.coyote_time)
    };
    if (s.on_ground() || coyote.is_some()) && wants_jump {
        // Apex removes an extra half tick of gravity on the jump tick: with the
        // air move's split gravity below, the position advances by
        // (V - g*dt)*dt and the tick ends at V - 1.5*g*dt. Measured on the S21
        // client at 60 and 144 fps (rise 53.5 vs 55.0); higher fps jumps higher.
        // Jump fatigue (measured): after landing from a jump, height is 30 %
        // until 0.14 s on the ground, then recovers linearly to full at 0.735 s.
        // Falls and walk-offs do not fatigue. Jumps out of a slide launch 0.944x.
        let frac = if coyote.is_none() && s.landed_from_jump {
            cfg.jump_fatigue_frac(s.time_on_ground)
        } else {
            1.0
        };
        let ground_time = coyote.map_or(s.time_on_ground, |c| c.ground_time);
        let from_slide = s.sliding || coyote.is_some_and(|c| c.from_slide);
        let mut launch = cfg.jump_speed(cfg.jump_height * frac);
        let speed = s.velocity.horizontal().length();
        // Bunnyhop penalty (S21 traces + wiki "skip_time"): jumping within
        // 1 s of landing cuts speed above sprint by up to 100, never below
        // sprint (443.3 -> 343.3, 337 -> 299, 297 kept; 1.13 s+ untouched).
        let bhop = ground_time < cfg.bhop_skip_time;
        let mut target = speed;
        if bhop && speed > cfg.sprint_speed {
            target = (speed - cfg.bhop_speed_penalty).max(cfg.sprint_speed);
        }
        // Deadslide (session 7 sweep): jumping before the slide has settled
        // (0.206 s, the log's duck state 1 -> 2) while above 350 gives back
        // that slide's boost: 460 -> 299 (-161.0) on every run up to 33
        // frames at 165 fps; from 36 frames the speed is kept.
        let deadslide = s.sliding
            && s.slide_t < cfg.deadslide_time
            && speed > cfg.slide_jump_min_speed
            && s.slide_boost_given > 0.0;
        if deadslide {
            target = (target - s.slide_boost_given).max(0.0);
        }
        if from_slide {
            launch *= cfg.slide_jump_mult;
            // Slide long jump: an unfatigued slide-jump is at least 350 u/s
            // horizontally (S21 traces: 316..335 -> 350.0; faster is kept).
            // Not on a bhop: 300.9 -> 299 at 0.8 s after landing.
            if !deadslide
                && !bhop
                && frac >= 1.0
                && speed > 1e-3
                && speed < cfg.slide_jump_min_speed
            {
                target = cfg.slide_jump_min_speed;
            }
        }
        if speed > 1e-3 && target != speed {
            let k = target / speed;
            s.velocity.x *= k;
            s.velocity.y *= k;
        }
        // A coyote jump keeps the vertical speed it already had (S21: all 10
        // coyote jumps end at vz_in + V - 1.5 g dt).
        let vz_in = if coyote.is_some() { s.velocity.z } else { 0.0 };
        s.velocity.z = vz_in + launch - 0.5 * cfg.gravity * dt;
        s.coyote = None;
        s.air_from_jump = true;
        s.mode = MoveMode::Air;
        s.ground_normal = Vec3::ZERO;
        s.sliding = false;
        rep.jumped = true;
    }

    let hull = cfg.hull(s.crouched);
    let pre_move_h = s.velocity.horizontal();

    // Pushing into a wall touched on an earlier tick: a mantle (ground or air)
    // or a wall climb (air) starts at the start of this tick and replaces the
    // normal move. S21 traces: mantles begin the tick after contact; the climb
    // attaches before gravity is applied.
    if !rep.jumped {
        let allow_climb = !s.on_ground();
        crate::traverse::try_start(
            &mut s,
            &mut rep,
            wish_dir,
            fwd,
            base_speed >= cfg.sprint_speed,
            pre_move_h,
            allow_climb,
            cfg,
            world,
            dt,
        );
        if s.climb.is_some() {
            // The attach tick itself was handled by try_start.
            finish_tick(&mut s, &mut rep, input, was_on_ground, cfg, world, dt);
            return (s, rep);
        }
        if s.mantle.is_some() {
            crate::traverse::mantle_tick(
                &mut s, &mut rep, input, pressed, wish_dir, cfg, world, dt,
            );
            finish_tick(&mut s, &mut rep, input, was_on_ground, cfg, world, dt);
            return (s, rep);
        }
    }
    if s.on_ground() && s.sliding {
        s.velocity.z = 0.0;
        let speed = s.velocity.length();
        if speed > 0.0 {
            // Crouch released above the stand-up speed: the slide goes on with
            // more decel (S21: +310 u/s², ends at 347-350).
            let released = !input.buttons.contains(Buttons::CROUCH);
            let extra = if released {
                cfg.slide_release_extra_decel
            } else {
                0.0
            };
            let decel = (cfg.slide_decel_const + cfg.slide_decel_linear * speed + extra) * dt;
            let new_speed = (speed - decel).max(0.0);
            s.velocity *= new_speed / speed;
            rep.friction_drop = speed - new_speed;
        }
        if cfg.slide_slope_gravity {
            // Gravity's pull along the slope, horizontal part: g·n.z·n_h
            // (31° ramp: 332 u/s²; S21 downhill +21..+45/s at 500-560,
            // uphill -577/s at 400 vs -573 predicted). It also steers the
            // slide downhill.
            let n = s.ground_normal;
            s.velocity.x += cfg.gravity * n.z * n.x * dt;
            s.velocity.y += cfg.gravity * n.z * n.y * dt;
        }
        clamp_velocity(&mut s.velocity, cfg);

        let moved = ground_move(
            world,
            hull,
            cfg,
            s.position,
            s.velocity,
            s.ground_normal,
            dt,
        );
        s.position = stay_on_ground(world, hull, cfg, moved.position);
        s.velocity = moved.velocity.horizontal();
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

        let moved = ground_move(
            world,
            hull,
            cfg,
            s.position,
            s.velocity,
            s.ground_normal,
            dt,
        );
        s.position = stay_on_ground(world, hull, cfg, moved.position);
        s.velocity = moved.velocity.horizontal();
        rep.hit_wall = moved.hit_wall;
        rep.wall_normal = moved.wall_normal;
    } else {
        // Split gravity around the move: exact for constant gravity (the
        // trajectory is the midpoint rule), and it is what Source does.
        s.velocity.z -= 0.5 * cfg.gravity * dt;
        // Lurch time = time since the last jump / first airborne tick (the
        // jump tick counts as 0); restarts on wall jumps and mantle exits.
        let t_air = if was_on_ground { 0.0 } else { s.lurch_t };
        rep.lurch_k = lurch(&mut s, input, wish_dir, t_air, cfg, dt);
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

    if rep.hit_wall {
        let n = rep.wall_normal.horizontal().normalize_or_zero();
        s.last_wall_normal = n;
        s.last_wall_impact = pre_move_h.dot(-n);
    } else {
        s.last_wall_normal = Vec3::ZERO;
        s.last_wall_impact = 0.0;
    }
    finish_tick(&mut s, &mut rep, input, was_on_ground, cfg, world, dt);
    (s, rep)
}

/// Shared end of every tick: ground check, landing bookkeeping, timers.
fn finish_tick(
    s: &mut MovementState,
    rep: &mut StepReport,
    input: &MovementInput,
    was_on_ground: bool,
    cfg: &MovementConfig,
    world: &impl CollisionWorld,
    dt: f32,
) {
    if s.mantle.is_some() || s.climb.is_some() {
        // Mid-traversal: scripted, never "on the ground".
        s.mode = MoveMode::Air;
        s.ground_normal = Vec3::ZERO;
    } else {
        categorize(s, cfg, world);
        if s.on_ground() {
            s.velocity.z = 0.0;
        }
    }
    rep.landed = !was_on_ground && !rep.jumped && s.on_ground();
    if rep.landed {
        s.landed_from_jump = s.air_from_jump;
        // Fall stun (session 8 walk-offs + wiki): strength from the landing
        // speed into the ground (slopes soften it), 0 below a 300 u fall,
        // full from 800 u. The landing clamps horizontal speed to the
        // stunned max speed: a full stun stops you dead (300 -> 0).
        let into = -rep.entry_velocity.dot(s.ground_normal);
        let height = into.max(0.0).powi(2) / (2.0 * cfg.gravity);
        let range = (cfg.fall_stun_full_height - cfg.fall_stun_min_height).max(1e-3);
        let strength = ((height - cfg.fall_stun_min_height) / range).clamp(0.0, 1.0);
        if strength > 0.0 {
            s.fall_stun = Some((0.0, strength));
            let cap = cfg.sprint_speed * fall_stun_factor(s.fall_stun, cfg);
            let h = s.velocity.horizontal();
            let speed = h.length();
            if speed > cap && speed > 0.0 {
                s.velocity.x *= cap / speed;
                s.velocity.y *= cap / speed;
            }
        }
    }
    if was_on_ground && !s.on_ground() && !rep.jumped {
        // Walked off a ledge (or started a mantle): not a jump. Coyote time
        // starts (not for mantles/climbs, and a climb ends it).
        s.air_from_jump = false;
        s.coyote = (s.mantle.is_none() && s.climb.is_none()).then_some(crate::state::Coyote {
            t: 0.0,
            from_slide: s.sliding,
            ground_time: s.time_on_ground,
        });
    } else if s.on_ground() || rep.jumped || s.climb.is_some() || s.mantle.is_some() {
        s.coyote = None;
    } else if let Some(c) = s.coyote.as_mut() {
        c.t += dt;
    }
    // Climb space follows the ground; in the air it only moves down (falling
    // below the baseline drags it along).
    if s.on_ground() {
        s.climb_base = s.position.z;
        s.last_attach_z = None;
    } else if s.position.z < s.climb_base {
        s.climb_base = s.position.z;
    }

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
    if s.on_ground() {
        s.lurch_t = 0.0;
        s.lurch_disabled = false;
    } else if was_on_ground || rep.jumped {
        s.lurch_t = 0.0;
    } else {
        s.lurch_t += dt;
    }
    s.time_since_slide_boost = (s.time_since_slide_boost + dt).min(f32::MAX / 2.0);
    if let Some((t, strength)) = s.fall_stun {
        let t = t + dt;
        s.fall_stun = (t < strength.sqrt() * cfg.fall_stun_duration).then_some((t, strength));
    }
    if s.sliding {
        s.slide_t += dt;
    }
    s.prev_buttons = input.buttons;
    s.prev_forward = input.forward;
    s.prev_right = input.right;
    s.tick += 1;
    s.time += f64::from(dt);
}

/// Max-speed multiplier during a fall stun: 1 − s·(1 − (t/T)²), so a full
/// stun recovers as t² (S21: 0.48 s -> 0.24, 0.97 s -> 0.94) and a strength-s
/// stun starts at 1 − s (0.276 -> 0.72 measured). It ends after √s·T.
pub fn fall_stun_factor(stun: Option<(f32, f32)>, cfg: &MovementConfig) -> f32 {
    match stun {
        Some((t, strength)) => {
            let u = (t / cfg.fall_stun_duration.max(1e-3)).min(1.0);
            1.0 - strength * (1.0 - u * u)
        }
        None => 1.0,
    }
}

/// Ground move that keeps the horizontal velocity on slopes: the move runs
/// along the ground plane (vertical part chosen so the velocity lies in it)
/// and the stored velocity stays horizontal. S21 (session 8, 31° ramp): vz
/// is logged 0 on slopes and sprinting/sliding up or down accelerates exactly
/// like on flat ground; a plain horizontal move clipped into the ramp lost
/// ~25 % of the speed and its upward clip velocity (> 140) left the ground.
fn ground_move(
    world: &impl CollisionWorld,
    hull: apex_math::Aabb,
    cfg: &MovementConfig,
    pos: Vec3,
    velocity: Vec3,
    n: Vec3,
    dt: f32,
) -> crate::collide::SlideResult {
    let h = velocity.horizontal();
    let along = if n.z > 0.0 {
        Vec3::new(h.x, h.y, -(n.x * h.x + n.y * h.y) / n.z)
    } else {
        h
    };
    step_slide_move(world, hull, cfg, pos, along, dt)
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
    dt: f32,
) -> f32 {
    if s.lurch_disabled {
        return 0.0;
    }
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
    // The change is capped (S21, every session: |Δv| tops out at 0.7 × 299 +
    // one tick of air accel: 217.6 @60 fps, 212.4 @144, 209.3/212.3 @165).
    // Past the cap the change is cut along the same line, losing speed:
    // session 7 (known angles, 260 u/s): 90° at 0.15 s -> 51.1° / 227.8
    // predicted, 51.0° / 227.5 measured; 120° -> 54.2° / 177.3 vs 54.3° / 177.0.
    let cap = cfg.lurch_max_delta_frac * cfg.sprint_speed + cfg.air_accel * cfg.run_speed * dt;
    let mut new_h = dir * speed;
    let delta = new_h - h;
    let len = delta.length();
    if len > cap {
        new_h = h + delta * (cap / len);
    }
    s.velocity.x = new_h.x;
    s.velocity.y = new_h.y;
    k
}

/// Slide entry/exit (ground only). Entry: crouch held, newly pressed or just
/// landed, at or above `slide_start_min_speed`. A slide started from a sprint
/// on the ground, off cooldown, adds `slide_boost` up to `slide_boost_cap`
/// (never reduces speed). Exit: crouch released or speed below `slide_end_speed`.
fn update_slide(
    s: &mut MovementState,
    rep: &mut StepReport,
    input: &MovementInput,
    pressed: Buttons,
    was_on_ground: bool,
    cfg: &MovementConfig,
    dt: f32,
) {
    if !s.on_ground() {
        s.sliding = false;
        return;
    }
    let held = input.buttons.contains(Buttons::CROUCH);
    let speed = s.velocity.horizontal().length();
    if !s.sliding
        && held
        && (pressed.contains(Buttons::CROUCH) || !was_on_ground || s.time_on_ground == 0.0)
        && speed >= cfg.slide_start_min_speed
    {
        s.sliding = true;
        s.crouched = true;
        s.slide_t = 0.0;
        s.slide_boost_given = 0.0;
        rep.slide_started = true;
        // Boost only for slides started from a sprint on the ground (re-slides
        // on landing are not boosted: 156 of 168 in the S21 traces).
        let sprinting =
            was_on_ground && input.buttons.contains(Buttons::SPRINT) && input.forward > 0.0;
        if sprinting && s.time_since_slide_boost >= cfg.slide_boost_cooldown {
            // That tick's sprint acceleration lands first, then +172.5 (S21:
            // 219.98 -> 394.24 at 60 fps, 232.2 -> 405.4 at 165), capped.
            let accelerated = speed + ground_accel_rate(speed, cfg) * dt;
            let boosted = (accelerated + cfg.slide_boost)
                .min(cfg.slide_boost_cap)
                .max(speed);
            s.velocity.x *= boosted / speed;
            s.velocity.y *= boosted / speed;
            s.slide_boost_given = boosted - accelerated.min(boosted);
            s.time_since_slide_boost = 0.0;
            rep.slide_boosted = true;
        }
    } else if s.sliding
        && ((!held && speed <= cfg.slide_stand_max_speed) || speed < cfg.slide_end_speed)
    {
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

/// Safety net (Source `CheckStuck` / IW4 `PM_CorrectAllSolid` in spirit): if
/// the hull starts the tick inside solid, move it to the nearest free spot,
/// preferring up, then sideways. Returns whether it had to.
fn unstick(s: &mut MovementState, cfg: &MovementConfig, world: &impl CollisionWorld) -> bool {
    let hull = cfg.hull(s.crouched);
    if world.hull_fits(s.position, hull) {
        return false;
    }
    for step in 1..=36 {
        let up = s.position + Vec3::new(0.0, 0.0, step as f32);
        if world.hull_fits(up, hull) {
            s.position = up;
            return true;
        }
    }
    for r in 1..=24 {
        let r = r as f32;
        for (dx, dy) in [
            (r, 0.0),
            (-r, 0.0),
            (0.0, r),
            (0.0, -r),
            (r, r),
            (-r, r),
            (r, -r),
            (-r, -r),
        ] {
            for dz in [0.0, 4.0, 12.0, 24.0] {
                let p = s.position + Vec3::new(dx, dy, dz);
                if world.hull_fits(p, hull) {
                    s.position = p;
                    return true;
                }
            }
        }
    }
    false
}

/// Put a fresh state on the ground if it starts within the ground probe of a
/// walkable floor (e.g. the first tick of a replay). Changes nothing else.
pub fn settle(s: &mut MovementState, cfg: &MovementConfig, world: &impl CollisionWorld) {
    categorize(s, cfg, world);
    if s.on_ground() {
        s.velocity.z = 0.0;
    }
}

/// Decide Ground vs Air by probing just below the feet (Source `CategorizePosition`).
fn categorize(s: &mut MovementState, cfg: &MovementConfig, world: &impl CollisionWorld) {
    // Airborne and still rising: never snapped back down. Source's 140 u/s
    // rule alone ate fatigued jumps (~142 u/s at takeoff fell below 140 on
    // the next tick, within the ground probe); S21 traces keep them airborne.
    let rising_in_air = s.mode == MoveMode::Air && s.velocity.z > 0.0;
    if s.velocity.z > cfg.non_jump_velocity || rising_in_air {
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
    // A slide keeps the crouched hull even after crouch is released (it goes
    // on until the stand-up speed).
    let want = input.buttons.contains(Buttons::CROUCH) || (s.sliding && s.on_ground());
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
