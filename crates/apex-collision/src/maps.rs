//! Built-in test geometry. Same map for unit tests, the replay tool and the
//! sandbox, so a trace recorded in one can be reproduced in the others.
//!
//! Layout (top view, +X right, +Y up, origin at the spawn point):
//!
//! ```text
//!   y=+600  ─────────────────── wall ───────────────────
//!           [stairs 8×16u]        [mantle blocks 40/60/80/100u]
//!   spawn ●  ───────── open floor (run lane) ─────────▶ +X      wall at x=+1500
//!           [ramp 30°]  [ramp 45°]   [narrow beams 16/32u]
//!   y=-600  ─────────────────── wall ───────────────────
//! ```

use apex_math::Vec3;

use crate::{Brush, BrushWorld};

/// Floor top surface is at z = 0.
pub const FLOOR_Z: f32 = 0.0;
/// Where a player should start.
pub const SPAWN: Vec3 = Vec3::new(0.0, 0.0, 0.0);

pub fn proving_ground() -> BrushWorld {
    let mut w = BrushWorld::new();
    let v = Vec3::new;

    // Floor and perimeter walls.
    w.add(Brush::cuboid(
        v(-2000.0, -2000.0, -32.0),
        v(2000.0, 2000.0, FLOOR_Z),
    ));
    w.add(Brush::cuboid(
        v(1500.0, -2000.0, 0.0),
        v(1532.0, 2000.0, 512.0),
    ));
    w.add(Brush::cuboid(
        v(-2032.0, -2000.0, 0.0),
        v(-2000.0, 2000.0, 512.0),
    ));
    w.add(Brush::cuboid(
        v(-2000.0, 600.0, 0.0),
        v(1500.0, 632.0, 512.0),
    ));
    w.add(Brush::cuboid(
        v(-2000.0, -632.0, 0.0),
        v(1500.0, -600.0, 512.0),
    ));

    // Stairs: 8 steps, 16u rise, 24u run, climbing toward +X.
    for i in 0..8 {
        let x0 = 100.0 + 24.0 * i as f32;
        w.add(Brush::cuboid(
            v(x0, 300.0, 0.0),
            v(x0 + 24.0, 420.0, 16.0 * (i + 1) as f32),
        ));
    }

    // Mantle-height blocks: 40 / 60 / 80 / 100 u tall.
    for (i, h) in [40.0, 60.0, 80.0, 100.0].into_iter().enumerate() {
        let x0 = 500.0 + 160.0 * i as f32;
        w.add(Brush::cuboid(v(x0, 300.0, 0.0), v(x0 + 96.0, 420.0, h)));
    }

    // Ramps toward +X: ~30° (walkable) and 45° (just walkable, normal.z ≈ 0.707).
    w.add(Brush::ramp_x(
        v(100.0, -420.0, 0.0),
        v(100.0 + 173.2, -300.0, 100.0),
    ));
    w.add(Brush::ramp_x(
        v(400.0, -420.0, 0.0),
        v(500.0, -300.0, 100.0),
    ));

    // Narrow beams at 48u: 16u and 32u wide.
    w.add(Brush::cuboid(
        v(700.0, -420.0, 0.0),
        v(1100.0, -404.0, 48.0),
    ));
    w.add(Brush::cuboid(
        v(700.0, -340.0, 0.0),
        v(1100.0, -308.0, 48.0),
    ));

    w
}
