//! Cast models: skeleton, skinned meshes and materials.

use crate::format::{CastError, Node, Value, parse};
use crate::xform::{QUAT_IDENTITY, V3, Xform};

#[derive(Clone, Debug, PartialEq)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    /// Rest pose relative to the parent.
    pub local: Xform,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Skeleton {
    pub bones: Vec<Bone>,
}

impl Skeleton {
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name == name)
    }

    pub fn rest_locals(&self) -> Vec<Xform> {
        self.bones.iter().map(|b| b.local).collect()
    }

    /// Model-space transforms from local ones. Cast lists parents before
    /// children; a parent that comes later is resolved lazily anyway.
    pub fn globals(&self, locals: &[Xform]) -> Vec<Xform> {
        let mut out: Vec<Option<Xform>> = vec![None; self.bones.len()];
        fn resolve(s: &Skeleton, locals: &[Xform], out: &mut [Option<Xform>], i: usize) -> Xform {
            if let Some(x) = out[i] {
                return x;
            }
            let x = match s.bones[i].parent {
                Some(p) if p != i && p < s.bones.len() => {
                    resolve(s, locals, out, p).mul(&locals[i])
                }
                _ => locals[i],
            };
            out[i] = Some(x);
            x
        }
        (0..self.bones.len())
            .map(|i| resolve(self, locals, &mut out, i))
            .collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Material {
    pub name: String,
    /// Cast material type (RSX writes "pbr").
    pub kind: String,
    /// Texture slot (albedo, normal, gloss, ...) -> file path as written by
    /// the exporter (relative to the model file's folder).
    pub textures: Vec<(String, String)>,
}

impl Material {
    pub fn texture(&self, slot: &str) -> Option<&str> {
        self.textures
            .iter()
            .find(|(s, _)| s == slot)
            .map(|(_, p)| p.as_str())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    pub name: String,
    pub positions: Vec<V3>,
    pub normals: Vec<V3>,
    pub uvs: Vec<[f32; 2]>,
    /// Bone influences per vertex (`weight_bones`/`weights` hold
    /// `influences` entries per vertex).
    pub influences: usize,
    pub weight_bones: Vec<u32>,
    pub weights: Vec<f32>,
    pub indices: Vec<u32>,
    pub material: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    pub skeleton: Skeleton,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}

fn v3s(n: &Node, name: &str) -> Vec<V3> {
    match n.prop(name) {
        Some(Value::V3(v)) => v.clone(),
        _ => Vec::new(),
    }
}

pub(crate) fn read_skeleton(skel: Option<&Node>) -> Skeleton {
    let Some(skel) = skel else {
        return Skeleton::default();
    };
    let bones = skel
        .children_of(b"bone")
        .map(|b| {
            let parent = b.prop("p").and_then(Value::first_u64).and_then(|p| {
                // -1 (stored as all bits set) = no parent.
                (p != u64::from(u32::MAX) && p != u64::MAX).then_some(p as usize)
            });
            let pos = match b.prop("lp") {
                Some(Value::V3(v)) => v.first().copied().unwrap_or([0.0; 3]),
                _ => [0.0; 3],
            };
            let rot = match b.prop("lr") {
                Some(Value::V4(v)) => v.first().copied().unwrap_or(QUAT_IDENTITY),
                _ => QUAT_IDENTITY,
            };
            Bone {
                name: b.str("n").unwrap_or_default().to_owned(),
                parent,
                local: Xform::new(rot, pos),
            }
        })
        .collect();
    Skeleton { bones }
}

impl Model {
    pub fn from_bytes(buf: &[u8]) -> Result<Model, CastError> {
        let roots = parse(buf)?;
        let modl = roots
            .iter()
            .find_map(|r| r.find(b"modl"))
            .ok_or(CastError::Missing("model node"))?;
        let skeleton = read_skeleton(modl.child(b"skel"));

        let mut material_hashes = Vec::new();
        let materials = modl
            .children_of(b"matl")
            .map(|m| {
                material_hashes.push(m.hash);
                let files: Vec<(u64, &str)> = m
                    .children_of(b"file")
                    .map(|f| (f.hash, f.str("p").unwrap_or_default()))
                    .collect();
                let textures = m
                    .props
                    .iter()
                    .filter_map(|(slot, v)| {
                        let Value::U64(h) = v else { return None };
                        let h = *h.first()?;
                        let path = files.iter().find(|(fh, _)| *fh == h)?.1;
                        Some((slot.clone(), path.to_owned()))
                    })
                    .collect();
                Material {
                    name: m.str("n").unwrap_or_default().to_owned(),
                    kind: m.str("t").unwrap_or_default().to_owned(),
                    textures,
                }
            })
            .collect();

        let meshes = modl
            .children_of(b"mesh")
            .map(|m| {
                let positions = v3s(m, "vp");
                let count = positions.len();
                let uvs = match m.prop("u0") {
                    Some(Value::V2(v)) => v.clone(),
                    _ => vec![[0.0; 2]; count],
                };
                let influences = m
                    .prop("mi")
                    .and_then(Value::first_u64)
                    .map_or(1, |x| x.max(1) as usize);
                let weight_bones = m.prop("wb").and_then(Value::as_u32s).unwrap_or_default();
                let weights = match m.prop("wv") {
                    Some(Value::F32(v)) => v.clone(),
                    _ => vec![1.0; weight_bones.len()],
                };
                let material = m
                    .prop("m")
                    .and_then(Value::first_u64)
                    .and_then(|h| material_hashes.iter().position(|&mh| mh == h));
                Mesh {
                    name: m.str("n").unwrap_or_default().to_owned(),
                    normals: v3s(m, "vn"),
                    positions,
                    uvs,
                    influences,
                    weight_bones,
                    weights,
                    indices: m.prop("f").and_then(Value::as_u32s).unwrap_or_default(),
                    material,
                }
            })
            .collect();
        Ok(Model {
            skeleton,
            meshes,
            materials,
        })
    }
}

/// CPU skinning: transform a mesh by per-bone skinning transforms
/// (`skin[i]` = current global of bone i × inverse rest global). `bone_map`
/// maps the mesh's own bone indices to indices into `skin` (identity for a
/// mesh skinned to the same skeleton). Normals are rotated, not renormalized
/// per influence; they are normalized at the end.
pub fn skin_mesh(
    mesh: &Mesh,
    skin: &[Xform],
    bone_map: &[usize],
    positions: &mut Vec<V3>,
    normals: &mut Vec<V3>,
) {
    positions.clear();
    normals.clear();
    let n = mesh.influences.max(1);
    for (v, p) in mesh.positions.iter().enumerate() {
        let nrm = mesh.normals.get(v).copied().unwrap_or([0.0, 0.0, 1.0]);
        let mut acc_p = [0.0f32; 3];
        let mut acc_n = [0.0f32; 3];
        let mut total = 0.0f32;
        for k in 0..n {
            let i = v * n + k;
            let (Some(&b), Some(&w)) = (mesh.weight_bones.get(i), mesh.weights.get(i)) else {
                continue;
            };
            if w <= 0.0 {
                continue;
            }
            let Some(x) = bone_map.get(b as usize).and_then(|&j| skin.get(j)) else {
                continue;
            };
            let sp = x.point(*p);
            let sn = x.vector(nrm);
            for a in 0..3 {
                acc_p[a] += sp[a] * w;
                acc_n[a] += sn[a] * w;
            }
            total += w;
        }
        if total <= 0.0 {
            positions.push(*p);
            normals.push(nrm);
            continue;
        }
        let pos = [acc_p[0] / total, acc_p[1] / total, acc_p[2] / total];
        let len = (acc_n[0] * acc_n[0] + acc_n[1] * acc_n[1] + acc_n[2] * acc_n[2])
            .sqrt()
            .max(1e-8);
        positions.push(pos);
        normals.push([acc_n[0] / len, acc_n[1] / len, acc_n[2] / len]);
    }
}
