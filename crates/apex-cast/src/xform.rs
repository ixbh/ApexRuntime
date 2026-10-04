//! Rigid transforms (unit quaternion + translation) for bones. Cast stores
//! bone rotations as quaternions `[x, y, z, w]`; bone scale is ignored
//! (Apex viewmodels do not scale bones).

pub type Quat = [f32; 4];
pub type V3 = [f32; 3];

pub const QUAT_IDENTITY: Quat = [0.0, 0.0, 0.0, 1.0];

pub fn quat_mul(a: Quat, b: Quat) -> Quat {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

pub fn quat_conj(q: Quat) -> Quat {
    [-q[0], -q[1], -q[2], q[3]]
}

pub fn quat_normalize(q: Quat) -> Quat {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if len > 0.0 {
        [q[0] / len, q[1] / len, q[2] / len, q[3] / len]
    } else {
        QUAT_IDENTITY
    }
}

pub fn quat_rotate(q: Quat, v: V3) -> V3 {
    // v + 2w(u x v) + 2u x (u x v), u = q.xyz
    let u = [q[0], q[1], q[2]];
    let w = q[3];
    let c1 = cross(u, v);
    let c2 = cross(u, c1);
    [
        v[0] + 2.0 * (w * c1[0] + c2[0]),
        v[1] + 2.0 * (w * c1[1] + c2[1]),
        v[2] + 2.0 * (w * c1[2] + c2[2]),
    ]
}

/// Normalized lerp along the shorter arc (keys are close together).
pub fn quat_nlerp(a: Quat, b: Quat, t: f32) -> Quat {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let s = if dot < 0.0 { -1.0 } else { 1.0 };
    quat_normalize([
        a[0] + (b[0] * s - a[0]) * t,
        a[1] + (b[1] * s - a[1]) * t,
        a[2] + (b[2] * s - a[2]) * t,
        a[3] + (b[3] * s - a[3]) * t,
    ])
}

pub fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn lerp3(a: V3, b: V3, t: f32) -> V3 {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// `p' = rot * p + pos`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xform {
    pub rot: Quat,
    pub pos: V3,
}

impl Default for Xform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Xform {
    pub const IDENTITY: Self = Self {
        rot: QUAT_IDENTITY,
        pos: [0.0; 3],
    };

    pub fn new(rot: Quat, pos: V3) -> Self {
        Self {
            rot: quat_normalize(rot),
            pos,
        }
    }

    /// `self ∘ child`: apply `child` first, then `self`.
    pub fn mul(&self, child: &Xform) -> Xform {
        let p = quat_rotate(self.rot, child.pos);
        Xform {
            rot: quat_normalize(quat_mul(self.rot, child.rot)),
            pos: [p[0] + self.pos[0], p[1] + self.pos[1], p[2] + self.pos[2]],
        }
    }

    pub fn inverse(&self) -> Xform {
        let inv = quat_conj(self.rot);
        let p = quat_rotate(inv, self.pos);
        Xform {
            rot: inv,
            pos: [-p[0], -p[1], -p[2]],
        }
    }

    pub fn point(&self, p: V3) -> V3 {
        let r = quat_rotate(self.rot, p);
        [r[0] + self.pos[0], r[1] + self.pos[1], r[2] + self.pos[2]]
    }

    pub fn vector(&self, v: V3) -> V3 {
        quat_rotate(self.rot, v)
    }

    /// Column-major 4x4 matrix (for GPU upload / engine transforms).
    pub fn to_cols(&self) -> [[f32; 4]; 4] {
        let x = self.vector([1.0, 0.0, 0.0]);
        let y = self.vector([0.0, 1.0, 0.0]);
        let z = self.vector([0.0, 0.0, 1.0]);
        [
            [x[0], x[1], x[2], 0.0],
            [y[0], y[1], y[2], 0.0],
            [z[0], z[1], z[2], 0.0],
            [self.pos[0], self.pos[1], self.pos[2], 1.0],
        ]
    }
}
