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
    view_fwd: Vec3,
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
    // Inside the climb space, and below the last attach point unless this is
    // a different wall (wiki). Falling counts too (climb out of a fall).
    let in_space = s.position.z - s.climb_base < cfg.climb_space_height;
    let may_reattach = match s.last_attach_z {
        None => true,
        Some(z) => s.position.z < z || n.dot(s.last_attach_normal) < cfg.climb_reattach_min_dot,
    };
    if allow_climb
        && !s.on_ground()
        && !s.crouched
        && in_space
        && may_reattach
        && vz_mid <= cfg.climb_attach_vz
        && view_fwd.dot(-n) >= cfg.climb_look_min_dot
    {
        s.climb = Some(Climb {
            normal: n,
            t: 0.0,
            peaked: false,
            peak_t: 0.0,
            attach_z: s.position.z,
        });
        s.last_attach_z = Some(s.position.z);
        s.last_attach_normal = n;
        s.sliding = false;
        // Speed along the wall is carried into the climb (wiki); the part
        // into the wall is not.
        let along = pre_move_h - n * pre_move_h.dot(n);
        s.velocity = along + Vec3::new(0.0, 0.0, vz_mid);
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
///
/// The movement input relative to the wall picks the kind of climb (wiki
/// "Climb", "Sideways climb - Wallrun"): W climbs up (the measured S21
/// profile), A/D pointing into the wall climbs sideways along it (a
/// wallrun), sideways pointing away slips down. The player is held against
/// the wall and follows its surface; speed along the wall is carried.
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
    let tangent = Vec3::new(-n.y, n.x, 0.0);
    let along_speed = s.velocity.horizontal().dot(tangent);
    let into = wish_dir.dot(-n);
    let side = wish_dir.dot(tangent);
    let (view_fwd, _) = apex_math::yaw_basis(input.yaw);

    let detach = |s: &mut MovementState, c_vel: Vec3, penalty: f32| {
        s.climb = None;
        s.velocity = c_vel;
        s.mode = MoveMode::Air;
        s.climb_base -= penalty;
    };
    let z_rel = s.position.z - s.climb_base;

    // Reached the climb-space cutoff or the attach offset: the end push
    // (S21: 7/7 W climbs end at 147.2-147.6 above the baseline).
    if z_rel >= cfg.climb_space_height || s.position.z - c.attach_z >= cfg.climb_attach_offset {
        let push = n * cfg.climb_end_push_out
            + tangent * along_speed
            + Vec3::new(0.0, 0.0, cfg.climb_end_push_up);
        detach(s, push, cfg.climb_detach_penalty);
        rep.climb_finished = true;
        return;
    }

    if pressed.contains(Buttons::JUMP) {
        // Wall jump by climb zone (wiki; S21 traces):
        // * mini (0-19 above the baseline): +28.21 u hop, 188 out (mini-bounce);
        // * green (19-47): vz = sqrt(a - b·z), the lower the higher (wall
        //   bounce: 380 at 24.7 u, 260 at 44.8 u);
        // * neutral: no gain, keeps the climb speed minus a tick of gravity
        //   (S21: 136.3 -> 121.4, 122.4 -> 107.3, 114.4 -> 98.9).
        // Push off the wall, plus 258 along the A/D input (bounce |h| =
        // 258·√2); faster speed already along the wall (a wallrun) is kept.
        let (vz, out, penalty) = if z_rel <= cfg.climb_mini_zone_top {
            let hop = cfg.jump_speed(cfg.wall_mini_jump_height);
            (
                hop.max(s.velocity.z),
                cfg.wall_mini_push,
                cfg.climb_detach_penalty,
            )
        } else if z_rel <= cfg.climb_green_zone_top {
            let v = (cfg.wall_bounce_a - cfg.wall_bounce_b * z_rel)
                .max(0.0)
                .sqrt();
            (
                v.max(s.velocity.z),
                cfg.wall_jump_push,
                2.0 * cfg.climb_detach_penalty,
            )
        } else {
            (
                s.velocity.z - cfg.gravity * dt,
                cfg.wall_jump_push,
                2.0 * cfg.climb_detach_penalty,
            )
        };
        let mut h = n * out;
        if input.right != 0.0 {
            let dir = (wish_dir - n * wish_dir.dot(n)).normalize_or_zero();
            let carried = (tangent * along_speed).dot(dir);
            h += dir * carried.max(cfg.wall_jump_push);
        } else {
            h += tangent * along_speed;
        }
        detach(s, h + Vec3::new(0.0, 0.0, vz), penalty);
        s.air_from_jump = true;
        rep.jumped = true;
        rep.wall_jump = true;
        return;
    }

    // Non-jump detaches (wiki): crouch, back, looking more than 45.57° away.
    if pressed.contains(Buttons::CROUCH)
        || input.forward < 0.0
        || view_fwd.dot(-n) < cfg.climb_look_min_dot
    {
        let v = s.velocity;
        detach(s, v, cfg.climb_detach_penalty);
        return;
    }

    let climbing_up = input.forward > 0.0 && into > 0.0;
    let sideways = !climbing_up && input.right != 0.0;

    // Reached a ledge with the input pointing at it: mantle over it. A
    // sideways climb looking past the ledge does not (wiki "No-ma climb").
    let hull = cfg.hull(s.crouched);
    if into >= cfg.mantle_input_min_dot {
        let max_h = cfg.mantle_max_height;
        if let Some((end, height)) = find_ledge(world, cfg, s.position, -n, max_h) {
            start_mantle(s, rep, end, height, -n, cfg);
            return;
        }
    }

    // Vertical speed.
    let mut vz = s.velocity.z;
    if climbing_up {
        // Measured S21 W climb profile (it reaches the cutoff after 51 ticks
        // at 60 fps from a jump).
        if !c.peaked {
            vz += cfg.climb_accel * dt;
            if vz >= cfg.climb_peak_speed {
                vz = cfg.climb_peak_speed;
                c.peaked = true;
                c.peak_t = c.t + dt;
            }
        } else {
            // Under the decaying ceiling: W pressed again late climbs back up
            // only to it (S21: 95 at 0.567 s, ceiling 93).
            let ceiling = cfg.climb_peak_speed
                * apex_math::exp(-cfg.climb_decay_rate * (c.t + dt - c.peak_t));
            vz = (vz + cfg.climb_accel * dt).min(ceiling);
        }
    } else if sideways && into >= 0.0 {
        // Wallrun: upward momentum runs out under gravity, falling is held
        // off by the climb, so the run stays roughly level.
        vz = if vz > 0.0 {
            // S21: -7.7/tick at 181 u/s, -11.3 at 37 (60 fps).
            (vz + (cfg.climb_side_vz_drag * vz - cfg.gravity) * dt).max(0.0)
        } else {
            (vz + cfg.climb_accel * dt).min(0.0)
        };
    } else {
        // Slipping (sideways input pointing away from the wall, or no input:
        // the brief "stick" wall bounces use): gravity only.
        vz -= cfg.gravity * dt;
    }

    // The ceiling peaks on time even when not climbing up.
    if !c.peaked && c.t + dt >= cfg.climb_peak_time {
        c.peaked = true;
        c.peak_t = cfg.climb_peak_time;
    }
    // Non-upward climbing has a 1 s timer (wiki).
    c.t += dt;
    if c.t >= cfg.climb_nonupward_time && vz < cfg.climb_nonupward_min_vz {
        detach(
            s,
            tangent * along_speed + Vec3::new(0.0, 0.0, vz),
            cfg.climb_detach_penalty,
        );
        return;
    }

    // Speed along the wall: toward the A/D input's target, faster carried
    // speed in that direction is kept; without A/D it bleeds off.
    let target = if sideways {
        side.signum() * cfg.climb_side_speed
    } else if climbing_up {
        side * cfg.climb_up_side_speed
    } else {
        0.0
    };
    let step = if climbing_up {
        cfg.climb_up_side_accel
    } else {
        cfg.climb_side_accel
    } * dt;
    let mut along = along_speed;
    // Only a sideways climb keeps faster carried speed (wiki); a W climb
    // eases down to its 115·share cap (S21 t=388.3: 92 -> ~75).
    let keeps_faster = sideways && along * target.signum() > target.abs();
    if !keeps_faster {
        along = if along < target {
            (along + step).min(target)
        } else {
            (along - step).max(target)
        };
    }

    // Move along the wall (held against it), then vertically.
    let tr = world.trace_hull(s.position, s.position + tangent * (along * dt), hull);
    if !tr.start_solid {
        if tr.fraction < 1.0 {
            // Ran into something along the wall (a corner): stop there.
            along = 0.0;
        }
        s.position = tr.end_pos;
    }
    let tr = world.trace_hull(s.position, s.position - n * (cfg.climb_suction * dt), hull);
    if !tr.start_solid {
        s.position = tr.end_pos;
    }
    let tr = world.trace_hull(s.position, s.position + Vec3::new(0.0, 0.0, vz * dt), hull);
    s.position = tr.end_pos;
    if tr.fraction < 1.0 {
        if vz < 0.0 {
            // Slipped down onto the floor: off the wall (the ground check
            // at the end of the tick lands us).
            detach(s, tangent * along, 0.0);
            return;
        }
        vz = 0.0;
    }

    // Follow the wall's surface; off its end: let go.
    match probe_wall(
        world,
        hull,
        s.position,
        -n,
        cfg.wall_probe_dist + cfg.climb_suction * dt,
    ) {
        Some(n2) if n2.dot(n) > 0.7 => c.normal = n2,
        _ => {
            // Off the side / top of the wall: no climb-space penalty.
            detach(s, tangent * along + Vec3::new(0.0, 0.0, vz), 0.0);
            return;
        }
    }

    s.velocity = tangent * along + Vec3::new(0.0, 0.0, vz);
    s.climb = Some(c);
}
