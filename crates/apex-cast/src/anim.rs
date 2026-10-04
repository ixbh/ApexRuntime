//! Cast animations: per-bone keyframe curves, sampled onto a skeleton.

use crate::format::{CastError, Value, parse};
use crate::model::Skeleton;
use crate::xform::{Quat, Xform, lerp3, quat_mul, quat_nlerp};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Tx,
    Ty,
    Tz,
    Rotation,
    /// Scale and visibility curves are read but not applied.
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Value replaces the bone's local transform component.
    Absolute,
    /// Value is relative to the rest pose (rest + t, rest ∘ r).
    Relative,
    /// Value is added on top of whatever is there (layers).
    Additive,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Keys {
    Scalar(Vec<f32>),
    Quat(Vec<Quat>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    pub bone: String,
    pub channel: Channel,
    pub mode: Mode,
    /// Frame numbers, ascending.
    pub frames: Vec<u32>,
    pub values: Keys,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NoteTrack {
    pub name: String,
    pub frames: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub fps: f32,
    pub looping: bool,
    pub curves: Vec<Curve>,
    pub notetracks: Vec<NoteTrack>,
    /// Last keyed frame.
    pub last_frame: u32,
}

impl Animation {
    pub fn from_bytes(buf: &[u8]) -> Result<Animation, CastError> {
        let roots = parse(buf)?;
        let anim = roots
            .iter()
            .find_map(|r| r.find(b"anim"))
            .ok_or(CastError::Missing("anim node"))?;
        let fps = anim
            .prop("fr")
            .and_then(Value::first_f32)
            .filter(|f| *f > 0.0)
            .unwrap_or(30.0);
        let looping = anim.prop("lo").and_then(Value::first_u64).unwrap_or(0) != 0;
        let mut last_frame = 0;
        let curves = anim
            .children_of(b"curv")
            .filter_map(|c| {
                let frames = c.prop("kb")?.as_u32s()?;
                let channel = match c.str("kp")? {
                    "tx" => Channel::Tx,
                    "ty" => Channel::Ty,
                    "tz" => Channel::Tz,
                    "rq" => Channel::Rotation,
                    _ => Channel::Other,
                };
                let values = match c.prop("kv")? {
                    Value::V4(v) => Keys::Quat(v.clone()),
                    Value::F32(v) => Keys::Scalar(v.clone()),
                    Value::F64(v) => Keys::Scalar(v.iter().map(|&x| x as f32).collect()),
                    _ => return None,
                };
                let mode = match c.str("m").unwrap_or("absolute") {
                    "relative" => Mode::Relative,
                    "additive" => Mode::Additive,
                    _ => Mode::Absolute,
                };
                last_frame = last_frame.max(frames.last().copied().unwrap_or(0));
                Some(Curve {
                    bone: c.str("nn").unwrap_or_default().to_owned(),
                    channel,
                    mode,
                    frames,
                    values,
                })
            })
            .collect();
        let notetracks = anim
            .children_of(b"nott")
            .map(|n| NoteTrack {
                name: n.str("n").unwrap_or_default().to_owned(),
                frames: n.prop("kb").and_then(Value::as_u32s).unwrap_or_default(),
            })
            .collect();
        Ok(Animation {
            fps,
            looping,
            curves,
            notetracks,
            last_frame,
        })
    }

    pub fn duration(&self) -> f32 {
        self.last_frame as f32 / self.fps
    }

    /// Curve -> bone index for `skeleton` (None = bone not in this skeleton).
    pub fn bind(&self, skeleton: &Skeleton) -> Vec<Option<usize>> {
        self.curves
            .iter()
            .map(|c| skeleton.index_of(&c.bone))
            .collect()
    }

    /// Write the pose at `time` seconds into `locals` (one per bone of the
    /// skeleton `binding` was made for; start from the rest pose or a lower
    /// layer). `weight` blends this animation over what is there (1 = replace).
    pub fn sample_into(
        &self,
        time: f32,
        binding: &[Option<usize>],
        rest: &[Xform],
        locals: &mut [Xform],
        weight: f32,
    ) {
        let frame = (time * self.fps).clamp(0.0, self.last_frame as f32);
        let w = weight.clamp(0.0, 1.0);
        for (curve, bone) in self.curves.iter().zip(binding) {
            let Some(b) = *bone else { continue };
            let (Some(x), Some(r)) = (locals.get_mut(b), rest.get(b)) else {
                continue;
            };
            let (i0, i1, t) = key_span(&curve.frames, frame);
            match (&curve.values, curve.channel) {
                (Keys::Quat(q), Channel::Rotation) => {
                    let (Some(&a), Some(&c)) = (q.get(i0), q.get(i1)) else {
                        continue;
                    };
                    let v = quat_nlerp(a, c, t);
                    let target = match curve.mode {
                        Mode::Absolute => v,
                        Mode::Relative => quat_mul(r.rot, v),
                        Mode::Additive => quat_mul(x.rot, v),
                    };
                    x.rot = quat_nlerp(x.rot, target, w);
                }
                (Keys::Scalar(s), ch @ (Channel::Tx | Channel::Ty | Channel::Tz)) => {
                    let (Some(&a), Some(&c)) = (s.get(i0), s.get(i1)) else {
                        continue;
                    };
                    let v = a + (c - a) * t;
                    let axis = match ch {
                        Channel::Tx => 0,
                        Channel::Ty => 1,
                        _ => 2,
                    };
                    let target = match curve.mode {
                        Mode::Absolute => v,
                        Mode::Relative => r.pos[axis] + v,
                        Mode::Additive => x.pos[axis] + v,
                    };
                    let mut p = x.pos;
                    p[axis] = target;
                    x.pos = lerp3(x.pos, p, w);
                }
                _ => {}
            }
        }
    }

    /// Notetracks that fire in the frame range (from, to] (seconds).
    pub fn notes_between(&self, from: f32, to: f32) -> impl Iterator<Item = &str> {
        let (a, b) = (from * self.fps, to * self.fps);
        self.notetracks.iter().filter_map(move |n| {
            n.frames
                .iter()
                .any(|&f| (f as f32) > a && (f as f32) <= b)
                .then_some(n.name.as_str())
        })
    }
}

/// Keys around `frame`: indices and blend factor.
fn key_span(frames: &[u32], frame: f32) -> (usize, usize, f32) {
    match frames.len() {
        0 => (0, 0, 0.0),
        1 => (0, 0, 0.0),
        n => {
            let next = frames.partition_point(|&f| (f as f32) <= frame);
            if next == 0 {
                (0, 0, 0.0)
            } else if next >= n {
                (n - 1, n - 1, 0.0)
            } else {
                let (f0, f1) = (frames[next - 1] as f32, frames[next] as f32);
                let t = if f1 > f0 {
                    (frame - f0) / (f1 - f0)
                } else {
                    0.0
                };
                (next - 1, next, t)
            }
        }
    }
}
