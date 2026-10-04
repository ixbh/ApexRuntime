//! Turn a convex brush (a set of planes) into a renderable mesh, so the
//! sandbox draws exactly the geometry the collision code sees.

use apex_collision::Brush;
use apex_math::Vec3 as SVec3;
use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};

use crate::to_bevy;

const EPS: f32 = 0.01;
/// One texture tile covers this many Source units (the grid texture draws a
/// line every 16 u and a bold one every 64 u).
pub const UNITS_PER_TILE: f32 = 64.0;

/// Corners of the brush: every point where three planes meet that lies inside all of them.
fn corners(brush: &Brush) -> Vec<SVec3> {
    let p = &brush.planes;
    let mut out: Vec<SVec3> = Vec::new();
    for i in 0..p.len() {
        for j in i + 1..p.len() {
            for k in j + 1..p.len() {
                let (a, b, c) = (p[i], p[j], p[k]);
                let denom = a.normal.dot(b.normal.cross(c.normal));
                if denom.abs() < 1e-6 {
                    continue;
                }
                let v = (b.normal.cross(c.normal) * a.dist
                    + c.normal.cross(a.normal) * b.dist
                    + a.normal.cross(b.normal) * c.dist)
                    / denom;
                let inside = p.iter().all(|q| q.normal.dot(v) <= q.dist + EPS);
                if inside && !out.iter().any(|o| (*o - v).length_sq() < EPS * EPS) {
                    out.push(v);
                }
            }
        }
    }
    out
}

pub fn brush_to_mesh(brush: &Brush) -> Mesh {
    let pts = corners(brush);
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for plane in &brush.planes {
        let mut face: Vec<SVec3> = pts
            .iter()
            .copied()
            .filter(|v| (plane.normal.dot(*v) - plane.dist).abs() < EPS)
            .collect();
        if face.len() < 3 {
            continue;
        }
        let n = plane.normal;
        let centre = face.iter().fold(SVec3::ZERO, |a, v| a + *v) / face.len() as f32;
        let seed = if n.z.abs() < 0.9 { SVec3::Z } else { SVec3::X };
        let u = seed.cross(n).normalize_or_zero();
        let w = n.cross(u);
        // Counter-clockwise seen from outside (u × w = n).
        face.sort_by(|a, b| {
            let ang = |p: &SVec3| {
                let d = *p - centre;
                d.dot(w).atan2(d.dot(u))
            };
            ang(a).total_cmp(&ang(b))
        });
        let base = positions.len() as u32;
        let bn = to_bevy(n).normalize_or_zero().to_array();
        // World-space UVs on the face's dominant plane, so grid lines sit at
        // exact world coordinates: on walls they mark heights every 16/64 u.
        let (ax, ay, az) = (n.x.abs(), n.y.abs(), n.z.abs());
        for v in &face {
            positions.push(to_bevy(*v).to_array());
            normals.push(bn);
            let (a, b) = if az >= ax && az >= ay {
                (v.x, v.y)
            } else if ax >= ay {
                (v.y, -v.z)
            } else {
                (v.x, -v.z)
            };
            uvs.push([a / UNITS_PER_TILE, b / UNITS_PER_TILE]);
        }
        for t in 1..face.len() as u32 - 1 {
            indices.extend_from_slice(&[base, base + t, base + t + 1]);
        }
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}
