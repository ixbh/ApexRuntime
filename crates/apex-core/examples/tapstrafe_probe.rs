//! Scripted tap strafe through the core: prints heading/speed per tick.
//! cargo run -q -p apex-core --example tapstrafe_probe -- <turn_deg> <notch_every_ticks> <hold_d 0|1>
use apex_collision::{Brush, BrushWorld};
use apex_core::*;

fn main() {
    let a: Vec<f32> = std::env::args()
        .skip(1)
        .map(|v| v.parse().unwrap())
        .collect();
    let (turn, every, hold_d) = (a[0], a[1] as u64, a[2] != 0.0);
    let dt = 1.0 / 144.0;
    let mut w = BrushWorld::new();
    w.add(Brush::cuboid(
        Vec3::new(-1e5, -1e5, -32.0),
        Vec3::new(1e5, 1e5, 0.0),
    ));
    let cfg = MovementConfig::default();
    let mut s = MovementState::at(Vec3::ZERO);
    settle(&mut s, &cfg, &w);
    s.velocity = Vec3::new(400.0, 0.0, 0.0);
    s.time_on_ground = 2.0;
    // jump (yaw 0 = +X)
    let jump = MovementInput {
        buttons: Buttons::JUMP,
        ..Default::default()
    };
    s = step(&s, &jump, &cfg, &w, dt);
    let ticks = (0.4 / dt) as u64;
    for t in 0..ticks {
        // Turning right = negative yaw in Source.
        let yaw = -turn * ((t + 1) as f32 / ticks as f32);
        let input = MovementInput {
            forward: if t % every == 0 { 1.0 } else { 0.0 },
            right: if hold_d { 1.0 } else { 0.0 },
            yaw,
            ..Default::default()
        };
        let (n, r) = step_with_report(&s, &input, &cfg, &w, dt);
        s = n;
        if t % 6 == 0 || t == ticks - 1 {
            let h = s.velocity.horizontal();
            println!(
                "t {:.3} view {:6.1} heading {:6.1} speed {:6.1} lurch_k {:.2}",
                (t + 1) as f32 * dt,
                yaw,
                h.y.atan2(h.x).to_degrees(),
                h.length(),
                r.lurch_k
            );
        }
    }
}
