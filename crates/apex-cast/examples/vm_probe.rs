//! Where does each viewmodel sequence put the gun relative to the camera?
//! `cargo run -p apex-cast --example vm_probe -- <gun.cast> <anim dir> [seq...]`
//! Prints the muzzle in eye space (+X left, +Y up, +Z forward) at the start
//! and middle of each sequence, plus how many curves it has and their modes.

use apex_cast::{Animation, Model, skinning};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let gun = Model::from_bytes(&std::fs::read(&args[0]).unwrap()).unwrap();
    let dir = std::path::Path::new(&args[1]);
    let rest = gun.skeleton.rest_locals();
    let rest_g = gun.skeleton.globals(&rest);
    let cam = gun.skeleton.index_of("jx_c_camera").unwrap();
    let muzzle = gun.skeleton.index_of("muzzle_flash").unwrap();
    let mut names: Vec<String> = args[2..].to_vec();
    if names.is_empty() {
        names = ["idle", "sprint", "reload_seq", "ads_in", "fire", "sprintslide", "lower", "jump", "land"]
            .iter()
            .map(|s| s.to_string())
            .collect();
    }
    let _ = skinning;
    for name in names {
        for suffix in ["_0", "_1"] {
            let path = dir.join(format!("{name}{suffix}.cast"));
            let Ok(bytes) = std::fs::read(&path) else { continue };
            let a = Animation::from_bytes(&bytes).unwrap();
            let bind = a.bind(&gun.skeleton);
            let mut modes = std::collections::BTreeMap::new();
            for c in &a.curves {
                *modes.entry(format!("{:?}", c.mode)).or_insert(0) += 1;
            }
            let keyed_cam = a.curves.iter().any(|c| c.bone == "jx_c_camera");
            let keyed_root = a.curves.iter().any(|c| c.bone == "ja_c_propGun" || c.bone == "def_c_base");
            let at = |t: f32| {
                let mut pose = rest.clone();
                a.sample_into(t, &bind, &rest, &mut pose, 1.0);
                let g = gun.skeleton.globals(&pose);
                let eye = g[cam].inverse();
                eye.point(g[muzzle].pos)
            };
            let fmt = |p: [f32; 3]| format!("({:6.1} {:6.1} {:6.1})", p[0], p[1], p[2]);
            let pov = gun.skeleton.index_of("jx_c_pov").unwrap();
            let pov_rel = |t: f32| {
                let mut pose = rest.clone();
                a.sample_into(t, &bind, &rest, &mut pose, 1.0);
                let g = gun.skeleton.globals(&pose);
                let eye = g[cam].inverse();
                (eye.point(g[pov].pos), eye.vector(g[pov].vector([0.0, 0.0, 1.0])))
            };
            let (pp, pf) = pov_rel(0.0);
            println!("    pov vs camera at t0: pos {} fwd {}", fmt(pp), fmt(pf));
            println!(
                "{name}{suffix:3} dur {:5.2}s loop {} curves {:4} {:?} cam-keyed {} gun-keyed {}  muzzle t0 {} mid {}",
                a.duration(),
                a.looping,
                a.curves.len(),
                modes,
                keyed_cam,
                keyed_root,
                fmt(at(0.0)),
                fmt(at(a.duration() * 0.5)),
            );
        }
    }
    let eye = rest_g[cam].inverse();
    let p = eye.point(rest_g[muzzle].pos);
    println!("rest pose muzzle ({:.1} {:.1} {:.1})", p[0], p[1], p[2]);
}
