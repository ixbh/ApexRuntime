//! World model bounds and bones: `cargo run -p apex-cast --example wm_probe -- <w_gun.cast>...`
use apex_cast::Model;

fn main() {
    for path in std::env::args().skip(1) {
        let m = Model::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        let mut verts = 0;
        for mesh in &m.meshes {
            verts += mesh.positions.len();
            for p in &mesh.positions {
                for i in 0..3 {
                    lo[i] = lo[i].min(p[i]);
                    hi[i] = hi[i].max(p[i]);
                }
            }
        }
        println!("{path}\n  meshes {} verts {verts} lo {lo:?} hi {hi:?}", m.meshes.len());
        let names: Vec<_> = m.meshes.iter().map(|x| x.name.as_str()).collect();
        println!("  mesh names {names:?}");
        let bones: Vec<_> = m.skeleton.bones.iter().map(|b| (b.name.as_str(), b.local.pos)).collect();
        println!("  bones {bones:?}");
    }
}
