//! apex-cast: reads Cast files (<https://github.com/dtzxporter/cast>), the
//! open model/animation format RSX exports Apex assets to. Skeletons,
//! skinned meshes, PBR material texture slots, animation curves with
//! notetracks, pose sampling and CPU skinning. No engine types: hosts (IW4L)
//! upload the skinned vertices themselves.
//!
//! Exported Apex content is personal-use only and lives outside the repos
//! (`F:\ApexContent`); nothing here embeds or ships any of it.

pub mod anim;
pub mod format;
pub mod model;
pub mod xform;

pub use anim::{Animation, Channel, Curve, Keys, Mode, NoteTrack};
pub use format::{CastError, Node, Value, parse};
pub use model::{Bone, Material, Mesh, Model, Skeleton, skin_mesh};
pub use xform::{Quat, V3, Xform};

/// Skinning transforms for a pose: `global(pose) ∘ global(rest)⁻¹` per bone.
pub fn skinning(skeleton: &Skeleton, rest_globals: &[Xform], locals: &[Xform]) -> Vec<Xform> {
    skeleton
        .globals(locals)
        .iter()
        .zip(rest_globals)
        .map(|(g, r)| g.mul(&r.inverse()))
        .collect()
}

/// Per-bone blend of two poses: `out = a·(1−w) + b·w` (rotations nlerped).
pub fn blend_poses(a: &[Xform], b: &[Xform], w: f32, out: &mut Vec<Xform>) {
    let w = w.clamp(0.0, 1.0);
    out.clear();
    out.extend(a.iter().zip(b).map(|(x, y)| Xform {
        rot: xform::quat_nlerp(x.rot, y.rot, w),
        pos: xform::lerp3(x.pos, y.pos, w),
    }));
}
