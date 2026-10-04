//! Traversal: mantle (with classic superglide and S21 mantle boost), wall
//! climb, wall jump / wall bounce. Rules and numbers from R5Flowstate S21
//! traces (sessions 4-5); see docs/MOVEMENT-MODEL.md and sheets/constants.tsv.
//!
//! The game does not log *inside* a mantle (TraversalMove), so the mantle
//! path between the measured start and end is a smooth stand-in; what is
//! measured is the duration (vs ledge height), the forward travel, and every
//! exit (normal, jump-off, superglide, mantle boost).

use apex_collision::CollisionWorld;
use apex_math::{Aabb, Vec3};

use crate::config::MovementConfig;
use crate::input::{Buttons, MovementInput};
use crate::state::{Climb, Mantle, MoveMode, MovementState};
use crate::step::StepReport;

/// Walls steeper than this (|normal.z| below) count for mantle/climb.
const WALL_MAX_NORMAL_Z: f32 = 0.3;

/// Horizontal wall normal if a wall lies within `dist` along `dir`.
pub fn probe_wall(
    world: &impl CollisionWorld,
    hull: Aabb,
    pos: Vec3,
    dir: Vec3,
    dist: f32,
) -> Option<Vec3> {
    if dir == Vec3::ZERO {
        return None;
    }
    let tr = world.trace_hull(pos, pos.mul_add(dist, dir), hull);
    if tr.fraction < 1.0 && !tr.start_solid && tr.normal.z.abs() < WALL_MAX_NORMAL_Z {
        let n = tr.normal.horizontal().normalize_or_zero();
        if n != Vec3::ZERO {
            return Some(n);
        }
    }
    None
}

/// A ledge in front of `pos` along `dir`: returns where the feet end up on top
/// (`mantle_forward` ahead, standing room checked) and the height above `pos`.
pub fn find_ledge(
    world: &impl CollisionWorld,
    cfg: &MovementConfig,
    pos: Vec3,
    dir: Vec3,
    max_height: f32,
) -> Option<(Vec3, f32)> {
    let stand = cfg.stand_hull;
    // Room to rise straight up from here.
    let ceiling_probe = world.trace_hull(
        pos,
        Vec3::new(pos.x, pos.y, pos.z + max_height + 1.0),
        stand,
    );
    if ceiling_probe.start_solid {
        return None;
    }
    let top_z = ceiling_probe.end_pos.z;
    // Room to move forward at that height.
    let over = Vec3::new(pos.x, pos.y, top_z);
    let ahead = over.mul_add(cfg.mantle_forward, dir);
    let across = world.trace_hull(over, ahead, stand);
    if across.fraction < 1.0 || across.start_solid {
        return None;
    }
    // Drop onto the top surface.
    let floor = Vec3::new(ahead.x, ahead.y, pos.z + cfg.step_height);
    let down = world.trace_hull(ahead, floor, stand);
    if down.fraction >= 1.0 || down.start_solid || down.normal.z < cfg.walkable_normal_z {
        return None;
    }
    let end = down.end_pos;
    let height = end.z - pos.z;
    if height <= cfg.step_height || height > max_height {
        return None;
    }
    Some((end, height))
}

fn start_mantle(
    s: &mut MovementState,
    rep: &mut StepReport,
    end: Vec3,
    height: f32,
    dir: Vec3,
    cfg: &MovementConfig,
) {
    let h = s.velocity.horizontal();
    let side_vel = h - dir * h.dot(dir);
    s.mantle = Some(Mantle {
        start: s.position,
        end,
        dir,
        t: 0.0,
        duration: cfg.mantle_duration_base + cfg.mantle_duration_per_unit * height,
        late_jump_ticks: None,
        side_vel,
        side_offset: Vec3::ZERO,
    });
    s.climb = None;
    s.sliding = false;
    s.velocity = Vec3::ZERO;
    s.mode = MoveMode::Air;
    s.ground_normal = Vec3::ZERO;
    rep.mantle_started = true;
}

/// Start a mantle or a wall climb if the player is pushing into a wall.
/// Called at the start of an airborne tick (so a climb attaches *before*
/// gravity, as in the S21 traces) and after every move (mantles only).
/// `pre_move_h` is the horizontal velocity before the move (the move itself
/// removes the part going into the wall).
#[allow(clippy::too_many_arguments)]
pub fn try_start(
    s: &mut MovementState,
    rep: &mut StepReport,
    wish_dir: Vec3,
    pre_move_h: Vec3,
    allow_climb: bool,
    cfg: &MovementConfig,
    world: &impl CollisionWorld,
    dt: f32,
) {
    if s.mantle.is_some() || s.climb.is_some() || rep.jumped {
        return;
    }
    let hull = cfg.hull(s.crouched);
    // Look for the wall along the input, the motion, and last tick's wall.
    let candidates = [
        wish_dir,
        pre_move_h.normalize_or_zero(),
        -s.last_wall_normal,
    ];
    let Some(n) = candidates
        .iter()
        .find_map(|&d| probe_wall(world, hull, s.position, d, cfg.wall_probe_dist))
    else {
        return;
    };
    // Pushing into it: any input toward it (A/D or a scroll-W tap count, W is
    // not required), current motion, or last tick's impact (the move that hit
    // the wall already removed the velocity into it).
    let hit_last_tick =
        s.last_wall_normal.dot(n) > 0.9 && s.last_wall_impact > cfg.wall_push_min_speed;
    let into_wall = wish_dir.dot(-n) > cfg.wall_push_min_dot
        || pre_move_h.dot(-n) > cfg.wall_push_min_speed
        || hit_last_tick;
    if !into_wall {
        return;
    }
    let max_h = if s.on_ground() {
        cfg.mantle_ground_max_height
    } else {
        cfg.mantle_max_height
    };
    if let Some((end, height)) = find_ledge(world, cfg, s.position, -n, max_h) {
        start_mantle(s, rep, end, height, -n, cfg);
        return;
    }
    // Climb attach is decided mid-tick, after the first half of gravity
    // (S21 traces: 95.9 -> 89.7 on the attach tick, the second half skipped).
    let vz_mid = s.velocity.z - 0.5 * cfg.gravity * dt;
    if allow_climb
        && !s.on_ground()
        && !s.climbed_this_air
        && !s.crouched
        && vz_mid > 0.0
        && vz_mid <= cfg.climb_attach_vz
    {
        s.climb = Some(Climb {
            normal: n,
            t: 0.0,
            peaked: false,
        });
        s.climbed_this_air = true;
        s.sliding = false;
        s.velocity = Vec3::new(0.0, 0.0, vz_mid);
        let tr = world.trace_hull(
            s.position,
            s.position + Vec3::new(0.0, 0.0, vz_mid * dt),
            hull,
        );
        s.position = tr.end_pos;
        rep.climb_started = true;
    }
}

fn exit_speed(input: &MovementInput, cfg: &MovementConfig) -> f32 {
    if input.buttons.contains(Buttons::SPRINT) && input.forward > 0.0 {
        cfg.sprint_speed
    } else {
        cfg.run_speed
    }
}

fn leave_mantle_airborne(
    s: &mut MovementState,
    rep: &mut StepReport,
    position: Vec3,
    velocity: Vec3,
) {
    s.mantle = None;
    s.position = position;
    s.velocity = velocity;
    s.mode = MoveMode::Air;
    s.air_from_jump = true;
    rep.jumped = true;
}

/// Classic superglide: jump, then crouch exactly one tick later, within the
/// last 0.15 s of the mantle (community superglide trainer; S21 traces). The
/// exit is a slide-jump from the ledge top: base speed + slide boost (capped)
/// and a full, unfatigued jump. Measured |h| 460.0 at 165 fps (299 + 174.4, cap).
fn superglide_exit(
    s: &mut MovementState,
    rep: &mut StepReport,
    m: &Mantle,
    input: &MovementInput,
    cfg: &MovementConfig,
    dt: f32,
) {
    let base = exit_speed(input, cfg);
    let h = if s.time_since_slide_boost >= cfg.slide_boost_cooldown {
        s.time_since_slide_boost = 0.0;
        (base + cfg.slide_boost).min(cfg.slide_boost_cap).max(base)
    } else {
        base
    };
    let vz = cfg.jump_speed(cfg.jump_height) - 0.5 * cfg.gravity * dt;
    let dir = (m.dir * h + m.side_vel).normalize_or_zero();
    leave_mantle_airborne(
        s,
        rep,
        m.end + m.side_offset,
        dir * h + Vec3::new(0.0, 0.0, vz),
    );
    s.crouched = true;
    s.crouch_frac = 1.0;
    rep.superglide = true;
}

/// S21 mantle boost: a late jump without the crouch. Measured |h| 388.7
/// (= 1.3 × 299) and a 90 u jump.
fn mantle_boost_exit(
    s: &mut MovementState,
    rep: &mut StepReport,
    m: &Mantle,
    input: &MovementInput,
    cfg: &MovementConfig,
    dt: f32,
) {
    let base = exit_speed(input, cfg);
    let (h, height) = if cfg.mantle_boost_enabled {
        rep.mantle_boost = true;
        (base * cfg.mantle_boost_mult, cfg.mantle_boost_jump_height)
    } else {
        (base, cfg.jump_height)
    };
    let vz = cfg.jump_speed(height) - 0.5 * cfg.gravity * dt;
    let dir = (m.dir * h + m.side_vel).normalize_or_zero();
    leave_mantle_airborne(
        s,
        rep,
        m.end + m.side_offset,
        dir * h + Vec3::new(0.0, 0.0, vz),
    );
}

/// One tick of an in-progress mantle.
#[allow(clippy::too_many_arguments)]
pub fn mantle_tick(
    s: &mut MovementState,
    rep: &mut StepReport,
    input: &MovementInput,
    pressed: Buttons,
    wish_dir: Vec3,
    cfg: &MovementConfig,
    world: &impl CollisionWorld,
    dt: f32,
) {
    let Some(mut m) = s.mantle else { return };

    // A late jump is pending: a crouch press exactly `superglide_crouch_ticks`
    // after it -> superglide; otherwise -> mantle boost.
    if let Some(n) = m.late_jump_ticks {
        if pressed.contains(Buttons::CROUCH) && f32::from(n) + 1.0 == cfg.superglide_crouch_ticks {
            superglide_exit(s, rep, &m, input, cfg, dt);
            return;
        }
        if f32::from(n) + 1.0 >= cfg.superglide_crouch_ticks {
            mantle_boost_exit(s, rep, &m, input, cfg, dt);
            return;
        }
        m.late_jump_ticks = Some(n + 1);
    }

    if pressed.contains(Buttons::JUMP) && m.late_jump_ticks.is_none() {
        let remaining = m.duration - m.t;
        if remaining <= cfg.superglide_window {
            // Crouch on this same tick does not count: the superglide needs
            // the crouch exactly one tick *after* the jump (decided next tick).
            if cfg.superglide_crouch_ticks <= 0.0 {
                mantle_boost_exit(s, rep, &m, input, cfg, dt);
                return;
            }
            m.late_jump_ticks = Some(0);
        } else if remaining <= cfg.mantle_pull_time
            || m.end.z - m.start.z <= cfg.mantle_grab_min_height
        {
            // In the pull but before the window: the press does nothing
            // (wiki: "if you jump too early you simply won't jump"), so
            // scroll-spamming into a ledge still mantles.
        } else {
            // Grab phase: jump straight off where we are (S21: vz of a full
            // jump, no horizontal speed; a mantle hop).
            let vz = cfg.jump_speed(cfg.jump_height) - 0.5 * cfg.gravity * dt;
            // The arc can overlap the ledge edge; step back out to the wall
            // plane the mantle started from if needed.
            let hull = cfg.hull(s.crouched);
            let mut here = s.position;
            if !world.hull_fits(here, hull) {
                let back = Vec3::new(
                    m.start.x + m.side_offset.x,
                    m.start.y + m.side_offset.y,
                    here.z,
                );
                if world.hull_fits(back, hull) {
                    here = back;
                }
            }
            leave_mantle_airborne(s, rep, here, m.side_vel + Vec3::new(0.0, 0.0, vz));
            return;
        }
    }

    let prev = s.position;
    m.t = (m.t + dt).min(m.duration);
    let u = if m.duration > 0.0 {
        m.t / m.duration
    } else {
        1.0
    };
    // Up-and-over arc: rise (ease-out) over the first 80 %, move onto the
    // ledge (smoothstep) over the last 80 %. Only the endpoints and the
    // duration are measured; the game does not log inside a mantle.
    let r = (u / 0.8).min(1.0);
    let rise = 1.0 - (1.0 - r) * (1.0 - r);
    let o = ((u - 0.2) / 0.8).clamp(0.0, 1.0);
    let over = o * o * (3.0 - 2.0 * o);

    // Sideways momentum along the wall: kept while A/D pushes that way,
    // otherwise it bleeds off. Moved with collision so it stops at obstacles.
    let side_input = wish_dir - m.dir * wish_dir.dot(m.dir);
    let speed = m.side_vel.length();
    if speed > 0.0 && side_input.dot(m.side_vel) <= 0.0 {
        let k = (speed - cfg.mantle_side_decay * dt).max(0.0) / speed;
        m.side_vel *= k;
    }
    let span = m.end - m.start;
    let base = Vec3::new(
        m.start.x + span.x * over,
        m.start.y + span.y * over,
        m.start.z + span.z * rise,
    );
    if m.side_vel != Vec3::ZERO {
        let hull = cfg.hull(s.crouched);
        let from = base + m.side_offset;
        let tr = world.trace_hull(from, from + m.side_vel * dt, hull);
        if tr.start_solid {
            m.side_offset += m.side_vel * dt;
        } else {
            m.side_offset = tr.end_pos - base;
            if tr.fraction < 1.0 {
                m.side_vel = Vec3::ZERO;
            }
        }
    }
    s.position = base + m.side_offset;
    s.velocity = if dt > 0.0 {
        (s.position - prev) / dt
    } else {
        Vec3::ZERO
    };

    if u >= 1.0 && m.late_jump_ticks.is_none() {
        // Finished: stand on the ledge, moving on at full speed if W is held.
        s.mantle = None;
        let hull = cfg.hull(s.crouched);
        let target = m.end + m.side_offset;
        let above = target + Vec3::new(0.0, 0.0, cfg.step_height);
        let below = target - Vec3::new(0.0, 0.0, cfg.step_height + 2.0);
        let tr = world.trace_hull(above, below, hull);
        s.position = if !tr.start_solid && tr.fraction < 1.0 {
            tr.end_pos
        } else if world.hull_fits(target, hull) {
            target
        } else {
            m.end
        };
        let max = exit_speed(input, cfg);
        let mut v = if input.forward > 0.0 {
            m.dir * max
        } else {
            Vec3::ZERO
        };
        v += m.side_vel;
        let cap = max.max(m.side_vel.length());
        if v.length() > cap {
            v = v.normalize_or_zero() * cap;
        }
        s.velocity = v;
        // Mantles do not cause jump fatigue.
        s.air_from_jump = false;
        rep.mantle_finished = true;
    } else {
        s.mantle = Some(m);
    }
}

/// One tick of an in-progress wall climb.
#[allow(clippy::too_many_arguments)]
pub fn climb_tick(
    s: &mut MovementState,
    rep: &mut StepReport,
    input: &MovementInput,
    pressed: Buttons,
    wish_dir: Vec3,
    cfg: &MovementConfig,
    world: &impl CollisionWorld,
    dt: f32,
) {
    let Some(mut c) = s.climb else { return };
    let n = c.normal;

    // Climb over (S21: 51 climb ticks at 60 fps, then the push on the next).
    if c.t + 1e-4 >= cfg.climb_duration {
        s.climb = None;
        s.velocity = n * cfg.climb_end_push_out + Vec3::new(0.0, 0.0, cfg.climb_end_push_up);
        rep.climb_finished = true;
        return;
    }

    if pressed.contains(Buttons::JUMP) {
        // Wall jump. Early (bounce window): vz ramps 260 -> 373 with contact
        // time; later: keeps the climb speed. Push 258 off the wall, plus 258
        // along the wall toward the A/D input (wall bounce |h| = 258·√2).
        let vz = if c.t <= cfg.wall_bounce_window {
            let a = (c.t / cfg.wall_bounce_window.max(1e-6)).clamp(0.0, 1.0);
            (cfg.wall_bounce_vz_min + (cfg.wall_bounce_vz_max - cfg.wall_bounce_vz_min) * a)
                .max(s.velocity.z)
        } else {
            // Late wall jump keeps the climb speed minus a tick of gravity
            // (S21: 136.3 -> 121.4, 122.4 -> 107.3, 114.4 -> 98.9).
            s.velocity.z - cfg.gravity * dt
        };
        let mut h = n * cfg.wall_jump_push;
        if input.right != 0.0 {
            let along = (wish_dir - n * wish_dir.dot(n)).normalize_or_zero();
            h += along * cfg.wall_jump_push;
        }
        s.climb = None;
        s.velocity = h + Vec3::new(0.0, 0.0, vz);
        s.mode = MoveMode::Air;
        s.air_from_jump = true;
        rep.jumped = true;
        rep.wall_jump = true;
        return;
    }

    // Without W this is only wall *contact* (what wall bounces use): no
    // climbing, and it lets go after the bounce window.
    if input.forward <= 0.0 {
        if c.t > cfg.wall_bounce_window {
            s.climb = None;
            return;
        }
        let vz = s.velocity.z - cfg.gravity * dt;
        let tr = world.trace_hull(
            s.position,
            s.position + Vec3::new(0.0, 0.0, vz * dt),
            cfg.hull(s.crouched),
        );
        s.position = tr.end_pos;
        s.velocity = Vec3::new(0.0, 0.0, vz);
        c.t += dt;
        s.climb = Some(c);
        return;
    }

    // Reached a ledge: mantle over it.
    if let Some((end, height)) = find_ledge(world, cfg, s.position, -n, cfg.mantle_max_height) {
        start_mantle(s, rep, end, height, -n, cfg);
        return;
    }
    // Lost the wall: fall.
    let hull = cfg.hull(s.crouched);
    if probe_wall(world, hull, s.position, -n, cfg.wall_probe_dist).is_none() {
        s.climb = None;
        return;
    }

    let mut vz = s.velocity.z;
    if !c.peaked {
        vz += cfg.climb_accel * dt;
        if vz >= cfg.climb_peak_speed {
            vz = cfg.climb_peak_speed;
            c.peaked = true;
        }
    } else {
        vz *= apex_math::exp(-cfg.climb_decay_rate * dt);
    }
    c.t += dt;

    let target = s.position + Vec3::new(0.0, 0.0, vz * dt);
    let tr = world.trace_hull(s.position, target, hull);
    s.position = tr.end_pos;
    if tr.fraction < 1.0 {
        vz = 0.0;
    }
    s.velocity = Vec3::new(0.0, 0.0, vz);
    s.climb = Some(c);
}
