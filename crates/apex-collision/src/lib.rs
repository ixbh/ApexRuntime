//! Collision queries for ApexRuntime.
//!
//! The movement core only ever talks to [`CollisionWorld`]. A host (the
//! sandbox, an IW4L adapter, a Unity plugin, ...) implements it on top of its
//! own physics. The query shape is the one Source/Respawn and IW4 movement are
//! built on: **sweep an axis-aligned box from `start` to `end` and report the
//! first blocking plane**. Respawn's player hull is an AABB, so matching this
//! shape is what lets wall/ground/step behaviour line up with Apex.
//!
//! [`BrushWorld`] is a self-contained reference implementation over convex
//! brushes (sets of planes). It is used by unit tests, the replay tool and the
//! sandbox, and doubles as executable documentation of the trace contract.

use apex_math::{Aabb, Vec3};

pub mod maps;

/// Distance the trace keeps from a surface it stops against (Source `DIST_EPSILON`).
/// Without it, the next move would start exactly on the plane and float error
/// would flip it to "inside".
pub const DIST_EPSILON: f32 = 0.03125;

/// Opaque per-surface tag supplied by the host (e.g. "mantle-able", "ladder",
/// "zipline"). The core gives meaning to specific bits later; 0 is plain solid.
pub type SurfaceTag = u32;

/// Result of a hull sweep.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Trace {
    /// Fraction of `start -> end` that was travelled before blocking (1.0 = clear).
    pub fraction: f32,
    /// Where the hull origin ended up (`start + (end - start) * fraction`).
    pub end_pos: Vec3,
    /// Normal of the blocking plane (zero if nothing was hit).
    pub normal: Vec3,
    /// The hull started inside solid.
    pub start_solid: bool,
    /// The hull was inside solid for the whole sweep (cannot move at all).
    pub all_solid: bool,
    /// Host tag of the surface that was hit.
    pub surface: SurfaceTag,
}

impl Trace {
    pub fn clear(start: Vec3, end: Vec3) -> Self {
        let _ = start;
        Self {
            fraction: 1.0,
            end_pos: end,
            normal: Vec3::ZERO,
            start_solid: false,
            all_solid: false,
            surface: 0,
        }
    }

    #[inline]
    pub fn hit(&self) -> bool {
        self.fraction < 1.0 || self.start_solid
    }
}

/// Everything the movement core needs from a host's physics.
pub trait CollisionWorld {
    /// Sweep `hull` (relative to the origin) from `start` to `end`.
    fn trace_hull(&self, start: Vec3, end: Vec3, hull: Aabb) -> Trace;

    /// Sweep a point (zero-size hull).
    fn trace_ray(&self, start: Vec3, end: Vec3) -> Trace {
        self.trace_hull(start, end, Aabb::new(Vec3::ZERO, Vec3::ZERO))
    }

    /// Is `hull` at `pos` free of solid?
    fn hull_fits(&self, pos: Vec3, hull: Aabb) -> bool {
        !self.trace_hull(pos, pos, hull).start_solid
    }
}

impl<T: CollisionWorld + ?Sized> CollisionWorld for &T {
    fn trace_hull(&self, start: Vec3, end: Vec3, hull: Aabb) -> Trace {
        (**self).trace_hull(start, end, hull)
    }
}

/// A world with nothing in it.
#[derive(Clone, Copy, Debug, Default)]
pub struct EmptyWorld;

impl CollisionWorld for EmptyWorld {
    fn trace_hull(&self, start: Vec3, end: Vec3, _hull: Aabb) -> Trace {
        Trace::clear(start, end)
    }
}

/// A plane `normal · p = dist`; the solid side is where `normal · p < dist`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Plane {
    pub normal: Vec3,
    pub dist: f32,
}

impl Plane {
    pub fn new(normal: Vec3, dist: f32) -> Self {
        Self { normal, dist }
    }

    pub fn through(normal: Vec3, point: Vec3) -> Self {
        let n = normal.normalize_or_zero();
        Self::new(n, n.dot(point))
    }
}

/// A convex solid: the intersection of the inner half-spaces of its planes.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Brush {
    pub planes: Vec<Plane>,
    /// Conservative bounds, used to skip brushes the sweep cannot touch.
    pub bounds: Aabb,
    pub surface: SurfaceTag,
}

impl Brush {
    /// An axis-aligned box.
    pub fn cuboid(mins: Vec3, maxs: Vec3) -> Self {
        let planes = vec![
            Plane::new(Vec3::X, maxs.x),
            Plane::new(-Vec3::X, -mins.x),
            Plane::new(Vec3::Y, maxs.y),
            Plane::new(-Vec3::Y, -mins.y),
            Plane::new(Vec3::Z, maxs.z),
            Plane::new(-Vec3::Z, -mins.z),
        ];
        Self {
            planes,
            bounds: Aabb::new(mins, maxs),
            surface: 0,
        }
    }

    /// A wedge inside `mins..maxs` whose top rises from `mins.z` at `x = mins.x`
    /// to `maxs.z` at `x = maxs.x` (a ramp climbing toward +X).
    ///
    /// All six axial planes are kept as bevels so box sweeps stay tight; the
    /// non-axial edge bevels are omitted, which can make a hull catch slightly
    /// early on the ramp's side edges. Fine for test geometry.
    pub fn ramp_x(mins: Vec3, maxs: Vec3) -> Self {
        let mut b = Self::cuboid(mins, maxs);
        let rise = maxs.z - mins.z;
        let run = maxs.x - mins.x;
        b.planes
            .push(Plane::through(Vec3::new(-rise, 0.0, run), mins));
        b
    }

    pub fn with_surface(mut self, tag: SurfaceTag) -> Self {
        self.surface = tag;
        self
    }

    /// Clip a hull sweep against this brush, tightening `tr` if it hits earlier.
    fn clip_sweep(&self, start: Vec3, end: Vec3, hull: Aabb, tr: &mut Trace) {
        let mut enter_frac = -1.0_f32;
        let mut leave_frac = 1.0_f32;
        let mut clip_normal = Vec3::ZERO;
        let mut starts_out = false;
        let mut gets_out = false;

        for plane in &self.planes {
            // Push the plane out by the hull's support point along -normal, so
            // the problem becomes a point (the hull origin) against the plane.
            let n = plane.normal;
            let offset = Vec3::new(
                if n.x < 0.0 { hull.maxs.x } else { hull.mins.x },
                if n.y < 0.0 { hull.maxs.y } else { hull.mins.y },
                if n.z < 0.0 { hull.maxs.z } else { hull.mins.z },
            );
            let dist = plane.dist - n.dot(offset);
            let d1 = start.dot(n) - dist;
            let d2 = end.dot(n) - dist;

            if d2 > 0.0 {
                gets_out = true;
            }
            // Exactly touching counts as outside (Quake uses `> 0`): hosts
            // spawn hulls flush on floors, and that must not read as stuck.
            if d1 >= 0.0 {
                starts_out = true;
            }
            // Entirely in front of this plane: the sweep misses the brush.
            if d1 >= 0.0 && (d2 >= DIST_EPSILON || d2 >= d1) {
                return;
            }
            // Entirely behind this plane: it does not limit the sweep.
            if d1 < 0.0 && d2 <= 0.0 {
                continue;
            }
            if d1 > d2 {
                // Entering the brush through this plane.
                let f = ((d1 - DIST_EPSILON) / (d1 - d2)).max(0.0);
                if f > enter_frac {
                    enter_frac = f;
                    clip_normal = n;
                }
            } else {
                // Leaving the brush through this plane.
                let f = ((d1 + DIST_EPSILON) / (d1 - d2)).min(1.0);
                if f < leave_frac {
                    leave_frac = f;
                }
            }
        }

        if !starts_out {
            tr.start_solid = true;
            tr.surface = self.surface;
            if !gets_out {
                tr.all_solid = true;
                tr.fraction = 0.0;
            }
            return;
        }
        if enter_frac < leave_frac && enter_frac > -1.0 && enter_frac < tr.fraction {
            tr.fraction = enter_frac.max(0.0);
            tr.normal = clip_normal;
            tr.surface = self.surface;
        }
    }
}

/// A list of convex brushes. Linear scan with bounds culling: plenty for test
/// maps and the sandbox; real maps will bring their own acceleration structure.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BrushWorld {
    pub brushes: Vec<Brush>,
}

impl BrushWorld {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, brush: Brush) -> &mut Self {
        self.brushes.push(brush);
        self
    }
}

impl CollisionWorld for BrushWorld {
    fn trace_hull(&self, start: Vec3, end: Vec3, hull: Aabb) -> Trace {
        let mut tr = Trace::clear(start, end);
        let pad = Vec3::splat(DIST_EPSILON * 2.0);
        let sweep = Aabb::new(
            start.min(end) + hull.mins - pad,
            start.max(end) + hull.maxs + pad,
        );
        for brush in &self.brushes {
            if !sweep.overlaps(brush.bounds) {
                continue;
            }
            brush.clip_sweep(start, end, hull, &mut tr);
            if tr.all_solid {
                break;
            }
        }
        tr.end_pos = if tr.fraction >= 1.0 {
            end
        } else {
            start.lerp(end, tr.fraction)
        };
        tr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hull() -> Aabb {
        Aabb::new(Vec3::new(-16.0, -16.0, 0.0), Vec3::new(16.0, 16.0, 72.0))
    }

    fn floor_world() -> BrushWorld {
        let mut w = BrushWorld::new();
        w.add(Brush::cuboid(
            Vec3::new(-1000.0, -1000.0, -16.0),
            Vec3::new(1000.0, 1000.0, 0.0),
        ));
        w
    }

    #[test]
    fn falling_hull_stops_just_above_floor() {
        let w = floor_world();
        let tr = w.trace_hull(
            Vec3::new(0.0, 0.0, 100.0),
            Vec3::new(0.0, 0.0, -100.0),
            hull(),
        );
        assert!(tr.fraction < 1.0);
        assert!(
            tr.end_pos.z > 0.0 && tr.end_pos.z < 0.1,
            "z = {}",
            tr.end_pos.z
        );
        assert_eq!(tr.normal, Vec3::Z);
        assert!(!tr.start_solid);
    }

    #[test]
    fn sweep_parallel_to_floor_is_clear() {
        let w = floor_world();
        let tr = w.trace_hull(Vec3::new(0.0, 0.0, 1.0), Vec3::new(500.0, 0.0, 1.0), hull());
        assert_eq!(tr.fraction, 1.0);
    }

    #[test]
    fn inside_solid_reports_all_solid() {
        let w = floor_world();
        let p = Vec3::new(0.0, 0.0, -8.0);
        let tr = w.trace_hull(p, p, Aabb::new(Vec3::ZERO, Vec3::ZERO));
        assert!(tr.start_solid && tr.all_solid);
    }

    #[test]
    fn wall_blocks_horizontal_sweep_with_correct_normal() {
        let mut w = BrushWorld::new();
        w.add(Brush::cuboid(
            Vec3::new(100.0, -500.0, 0.0),
            Vec3::new(132.0, 500.0, 200.0),
        ));
        let tr = w.trace_hull(Vec3::new(0.0, 0.0, 1.0), Vec3::new(300.0, 0.0, 1.0), hull());
        assert!(tr.fraction < 1.0);
        assert_eq!(tr.normal, -Vec3::X);
        // Hull front face (x + 16) stops just short of the wall at x = 100.
        assert!(tr.end_pos.x + 16.0 <= 100.0 && tr.end_pos.x + 16.0 > 99.9);
    }

    #[test]
    fn ramp_normal_points_up_and_back() {
        let mut w = BrushWorld::new();
        w.add(Brush::ramp_x(
            Vec3::new(0.0, -100.0, 0.0),
            Vec3::new(200.0, 100.0, 100.0),
        ));
        let tr = w.trace_ray(Vec3::new(100.0, 0.0, 200.0), Vec3::new(100.0, 0.0, -10.0));
        assert!(tr.fraction < 1.0);
        assert!(tr.normal.z > 0.8 && tr.normal.x < 0.0, "{:?}", tr.normal);
        assert!((tr.end_pos.z - 50.0).abs() < 0.1);
    }
}
