//! Moving a hull through the world: slide along planes, step up ledges,
//! stick to the ground. Source-lineage algorithms, written from their
//! published behaviour; Apex is expected to deviate in details (to be
//! established by measurement).

use apex_collision::CollisionWorld;
use apex_math::{Aabb, Vec3};

use crate::config::MovementConfig;

/// Planes remembered while resolving one move (Source `MAX_CLIP_PLANES`).
const MAX_CLIP_PLANES: usize = 5;
/// Sweeps per move before giving up (Source `numbumps`).
const MAX_BUMPS: usize = 4;

/// Remove the part of `v` going into a plane. `overbounce` > 1 pushes off it.
pub fn clip_velocity(v: Vec3, normal: Vec3, overbounce: f32) -> Vec3 {
    let backoff = v.dot(normal) * overbounce;
    let mut out = v - normal * backoff;
    // Make sure we really are no longer moving into the plane.
    let adjust = out.dot(normal);
    if adjust < 0.0 {
        out -= normal * adjust;
    }
    out
}

/// Outcome of [`slide_move`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SlideResult {
    pub position: Vec3,
    pub velocity: Vec3,
    /// Hit something walkable-ish (normal.z > 0.7).
    pub hit_floor: bool,
    /// Hit something steep (wall).
    pub hit_wall: bool,
    /// Last steep normal touched, for future wall mechanics.
    pub wall_normal: Vec3,
}

/// Move `position` by `velocity * dt`, sliding along every plane hit
/// (Source `TryPlayerMove`).
pub fn slide_move(
    world: &impl CollisionWorld,
    hull: Aabb,
    cfg: &MovementConfig,
    mut position: Vec3,
    mut velocity: Vec3,
    dt: f32,
) -> SlideResult {
    let primal_velocity = velocity;
    let mut original_velocity = velocity;
    let mut planes: [Vec3; MAX_CLIP_PLANES] = [Vec3::ZERO; MAX_CLIP_PLANES];
    let mut num_planes = 0usize;
    let mut time_left = dt;
    let mut res = SlideResult::default();

    for _ in 0..MAX_BUMPS {
        if velocity.length_sq() == 0.0 {
            break;
        }
        let end = position.mul_add(time_left, velocity);
        let tr = world.trace_hull(position, end, hull);

        if tr.all_solid {
            // Stuck inside something; don't make it worse.
            velocity = Vec3::ZERO;
            break;
        }
        if tr.fraction > 0.0 {
            position = tr.end_pos;
            original_velocity = velocity;
            num_planes = 0;
        }
        if tr.fraction >= 1.0 {
            break;
        }

        if tr.normal.z > cfg.walkable_normal_z {
            res.hit_floor = true;
        }
        if tr.normal.z.abs() < cfg.walkable_normal_z {
            res.hit_wall = true;
            res.wall_normal = tr.normal;
        }

        time_left -= time_left * tr.fraction;

        if num_planes >= MAX_CLIP_PLANES {
            velocity = Vec3::ZERO;
            break;
        }
        planes[num_planes] = tr.normal;
        num_planes += 1;

        // Find a velocity that slides along one plane without entering any other.
        let mut found = None;
        for i in 0..num_planes {
            let candidate = clip_velocity(original_velocity, planes[i], 1.0);
            let ok = (0..num_planes).all(|j| j == i || candidate.dot(planes[j]) >= 0.0);
            if ok {
                found = Some(candidate);
                break;
            }
        }
        match found {
            Some(v) => velocity = v,
            None => {
                if num_planes != 2 {
                    velocity = Vec3::ZERO;
                    break;
                }
                // Wedged between two planes: slide along their crease.
                let crease = planes[0].cross(planes[1]).normalize_or_zero();
                velocity = crease * crease.dot(velocity);
            }
        }

        // Turned back on ourselves (inside corner): stop dead to avoid jitter.
        if velocity.dot(primal_velocity) <= 0.0 {
            velocity = Vec3::ZERO;
            break;
        }
    }

    res.position = position;
    res.velocity = velocity;
    res
}

/// Ground move with stair stepping (Source `WalkMove` + `StepMove`).
///
/// Tries the plain slide and a "lift by step height, slide, drop back" slide,
/// and keeps whichever got further horizontally.
pub fn step_slide_move(
    world: &impl CollisionWorld,
    hull: Aabb,
    cfg: &MovementConfig,
    position: Vec3,
    velocity: Vec3,
    dt: f32,
) -> SlideResult {
    let dest = position.mul_add(dt, velocity);
    let direct = world.trace_hull(position, dest, hull);
    if direct.fraction >= 1.0 && !direct.start_solid {
        return SlideResult {
            position: dest,
            velocity,
            ..SlideResult::default()
        };
    }

    let down = slide_move(world, hull, cfg, position, velocity, dt);

    // Lift.
    let lift_to = position + Vec3::new(0.0, 0.0, cfg.step_height);
    let lift = world.trace_hull(position, lift_to, hull);
    if lift.all_solid {
        return down;
    }
    let mut up = slide_move(world, hull, cfg, lift.end_pos, velocity, dt);

    // Drop back down onto the step.
    let drop_to = up.position
        - Vec3::new(
            0.0,
            0.0,
            cfg.step_height + 2.0 * apex_collision::DIST_EPSILON,
        );
    let drop = world.trace_hull(up.position, drop_to, hull);
    if drop.all_solid || (drop.fraction < 1.0 && drop.normal.z < cfg.walkable_normal_z) {
        return down;
    }
    if drop.fraction >= 1.0 {
        // Stepped off into the air; the lift bought nothing.
        return down;
    }
    up.position = drop.end_pos;

    let d_down = (down.position - position).horizontal().length_sq();
    let d_up = (up.position - position).horizontal().length_sq();
    if d_down >= d_up {
        down
    } else {
        // Keep vertical velocity from the plain move (Source does the same).
        up.velocity.z = down.velocity.z;
        up
    }
}

/// Keep a grounded hull glued to the floor when walking down stairs/slopes.
pub fn stay_on_ground(
    world: &impl CollisionWorld,
    hull: Aabb,
    cfg: &MovementConfig,
    position: Vec3,
) -> Vec3 {
    let start = position + Vec3::new(0.0, 0.0, 2.0);
    let probe = world.trace_hull(position, start, hull);
    let start = probe.end_pos;
    let end = position - Vec3::new(0.0, 0.0, cfg.step_height);
    let tr = world.trace_hull(start, end, hull);
    if tr.fraction > 0.0
        && tr.fraction < 1.0
        && !tr.start_solid
        && tr.normal.z >= cfg.walkable_normal_z
        && (tr.end_pos.z - position.z).abs() > 0.5 * apex_collision::DIST_EPSILON
    {
        tr.end_pos
    } else {
        position
    }
}
