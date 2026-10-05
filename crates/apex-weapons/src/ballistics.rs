//! Bullet flight. Apex bullets are projectiles: they leave the muzzle at the
//! gun's launch speed, fall under scaled gravity and slow with drag. The
//! host sweeps each tick's segment through its world and hitboxes.

use crate::spec::Ballistics;

pub type V3 = [f32; 3];

/// Apex world gravity, units/s² (the movement gravity).
pub const GRAVITY: f32 = 750.0;

/// Drag model (estimate, to be measured): dv/dt = -drag * |v| * v / 1000.
pub const DRAG_SCALE: f32 = 1.0 / 1000.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bolt {
    pub pos: V3,
    pub vel: V3,
    /// Distance travelled so far (damage falloff uses it).
    pub travelled: f32,
    pub age: f32,
    /// Gun table index, owner id and charge fraction, for the hit.
    pub gun: usize,
    pub owner: u32,
    pub charge: f32,
}

/// Bolts older than this are dropped (seconds).
pub const MAX_AGE: f32 = 4.0;

fn len(v: V3) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

impl Bolt {
    pub fn launch(
        origin: V3,
        dir: V3,
        b: &Ballistics,
        gun: usize,
        owner: u32,
        charge: f32,
    ) -> Bolt {
        let l = len(dir).max(1e-6);
        let speed = b.speed;
        Bolt {
            pos: origin,
            vel: [dir[0] / l * speed, dir[1] / l * speed, dir[2] / l * speed],
            travelled: 0.0,
            age: 0.0,
            gun,
            owner,
            charge,
        }
    }

    /// Advance by `dt`; returns the segment flown (`from`, `to`) for the
    /// host to sweep. Z is up.
    pub fn step(&mut self, b: &Ballistics, dt: f32) -> (V3, V3) {
        let from = self.pos;
        let speed = len(self.vel);
        let drag = b.drag * speed * DRAG_SCALE;
        // Half the velocity change before and after the move (midpoint).
        let accel = |v: V3| {
            [
                -drag * v[0],
                -drag * v[1],
                -drag * v[2] - GRAVITY * b.gravity_scale,
            ]
        };
        let a0 = accel(self.vel);
        let mid = [
            self.vel[0] + a0[0] * dt * 0.5,
            self.vel[1] + a0[1] * dt * 0.5,
            self.vel[2] + a0[2] * dt * 0.5,
        ];
        let to = [
            from[0] + mid[0] * dt,
            from[1] + mid[1] * dt,
            from[2] + mid[2] * dt,
        ];
        let a1 = accel(mid);
        self.vel = [
            self.vel[0] + a1[0] * dt,
            self.vel[1] + a1[1] * dt,
            self.vel[2] + a1[2] * dt,
        ];
        self.pos = to;
        self.travelled += len([to[0] - from[0], to[1] - from[1], to[2] - from[2]]);
        self.age += dt;
        (from, to)
    }

    pub fn expired(&self) -> bool {
        self.age > MAX_AGE
    }
}

/// Aim direction for a pellet: the view forward rotated by `offset`
/// degrees `[right, up]`, given the view basis (forward, right, up).
pub fn pellet_dir(forward: V3, right: V3, up: V3, offset: [f32; 2]) -> V3 {
    let (r, u) = (offset[0].to_radians().tan(), offset[1].to_radians().tan());
    [
        forward[0] + right[0] * r + up[0] * u,
        forward[1] + right[1] * r + up[1] * u,
        forward[2] + right[2] * r + up[2] * u,
    ]
}

/// Where a bullet hit, for damage scaling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitZone {
    Head,
    Body,
    Legs,
}

/// Damage of one pellet hitting `zone` after flying `travelled` units.
pub fn pellet_damage(spec: &crate::GunSpec, travelled: f32, zone: HitZone, charge: f32) -> f32 {
    let base = spec.damage.at(travelled) * charge.clamp(0.0, 1.0).max(0.2);
    match zone {
        HitZone::Head => base * spec.damage.head,
        HitZone::Body => base,
        HitZone::Legs => base * spec.damage.leg,
    }
}
