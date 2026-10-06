//! Note tracks of animation sequences: `cargo run -p apex-cast --example notes_probe -- <seq.cast>...`
use apex_cast::Animation;

fn main() {
    for path in std::env::args().skip(1) {
        let a = Animation::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
        println!("{path}  ({:.3}s @ {} fps)", a.duration(), a.fps);
        let mut notes: Vec<(u32, &str)> = a
            .notetracks
            .iter()
            .flat_map(|n| n.frames.iter().map(move |&f| (f, n.name.as_str())))
            .collect();
        notes.sort();
        for (f, name) in notes {
            println!("  {:>7.3}s  {name}", f as f32 / a.fps);
        }
    }
}
