//! Behavioural tests for the Phase-1 core. These pin *architecture* properties
//! (determinism, frame-rate independence, collision sanity) and the placeholder
//! rules. When rules are replaced with measured Apex behaviour, the numeric
//! expectations change; the properties must not.

use apex_collision::maps::{SPAWN, proving_ground};
use apex_collision::{Brush, BrushWorld, EmptyWorld};
use apex_core::*;

const DT: f32 = 1.0 / 60.0;

fn flat() -> BrushWorld {
    let mut w = BrushWorld::new();
    w.add(Brush::cuboid(
        Vec3::new(-5000.0, -5000.0, -32.0),
        Vec3::new(5000.0, 5000.0, 0.0),
    ));
    w
}

fn run(
    world: &impl CollisionWorld,
    cfg: &MovementConfig,
    mut s: MovementState,
    ticks: usize,
    mut input: impl FnMut(usize) -> MovementInput,
) -> MovementState {
    for t in 0..ticks {
        s = step(&s, &input(t), cfg, world, DT);
    }
    s
}

fn settled(world: &impl CollisionWorld, cfg: &MovementConfig, at: Vec3) -> MovementState {
    let s = run(world, cfg, MovementState::at(at), 5, |_| {
        MovementInput::default()
    });
    assert!(s.on_ground(), "spawn should settle onto the floor");
    s
}

#[test]
fn identical_inputs_give_bit_identical_results() {
    let w = proving_ground();
    let cfg = MovementConfig::default();
    let script = |t: usize| MovementInput {
        forward: 1.0,
        right: if (t / 30).is_multiple_of(2) {
            0.5
        } else {
            -0.5
        },
        yaw: (t as f32) * 0.7,
        buttons: if t.is_multiple_of(45) {
            Buttons::JUMP
        } else {
            Buttons::SPRINT
        },
        ..MovementInput::default()
    };
    let a = run(&w, &cfg, MovementState::at(SPAWN), 600, script);
    let b = run(&w, &cfg, MovementState::at(SPAWN), 600, script);
    assert_eq!(a, b);
    assert_eq!(a.position.x.to_bits(), b.position.x.to_bits());
    assert_eq!(a.tick, 600);
}

#[test]
fn ground_acceleration_reaches_but_never_exceeds_run_speed() {
    let w = flat();
    let cfg = MovementConfig::default();
    let mut s = settled(&w, &cfg, Vec3::new(0.0, 0.0, 1.0));
    let mut max_seen = 0.0_f32;
    for _ in 0..120 {
        s = step(&s, &MovementInput::forward(), &cfg, &w, DT);
        max_seen = max_seen.max(s.horizontal_speed());
        assert!(s.on_ground());
    }
    assert!(
        (s.horizontal_speed() - cfg.run_speed).abs() < 1.0,
        "speed {}",
        s.horizontal_speed()
    );
    assert!(max_seen <= cfg.run_speed + 1e-3);
    assert!(
        s.position.x > 0.0 && s.position.y.abs() < 1e-3,
        "moved along +X"
    );
}

#[test]
fn sprint_is_faster_than_run() {
    let w = flat();
    let cfg = MovementConfig::default();
    let s0 = settled(&w, &cfg, Vec3::ZERO);
    let run_s = run(&w, &cfg, s0, 120, |_| MovementInput::forward());
    let sprint_s = run(&w, &cfg, s0, 120, |_| {
        MovementInput::forward().with_buttons(Buttons::SPRINT)
    });
    assert!((sprint_s.horizontal_speed() - cfg.sprint_speed).abs() < 1.0);
    assert!(sprint_s.position.x > run_s.position.x);
}

#[test]
fn friction_brings_player_to_rest() {
    let w = flat();
    let cfg = MovementConfig::default();
    let mut s = settled(&w, &cfg, Vec3::ZERO);
    s.velocity = Vec3::new(300.0, 0.0, 0.0);
    let s = run(&w, &cfg, s, 120, |_| MovementInput::default());
    assert_eq!(s.velocity, Vec3::ZERO);
}

/// Simulated jump rise at a given step length.
fn jump_rise(dt: f32) -> f32 {
    let w = flat();
    let cfg = MovementConfig::default();
    let mut s = settled(&w, &cfg, Vec3::ZERO);
    let ground_z = s.position.z;
    let mut peak = ground_z;
    for t in 0..2000 {
        let input = if t == 0 {
            MovementInput::default().with_buttons(Buttons::JUMP)
        } else {
            MovementInput::default()
        };
        let (next, rep) = step_with_report(&s, &input, &cfg, &w, dt);
        assert!(t != 0 || rep.jumped);
        s = next;
        peak = peak.max(s.position.z);
        if t > 0 && rep.landed {
            return peak - ground_z;
        }
    }
    panic!("never landed");
}

#[test]
fn jump_rise_matches_flowstate_at_60_and_144_fps() {
    // Golden values from the R5Flowstate S21 client (session 2026-10-02):
    // 3 jumps each, rise 53.52-53.60 at fps_max 60, 54.998-55.013 at 144.
    let r60 = jump_rise(1.0 / 60.0);
    let r144 = jump_rise(1.0 / 144.0);
    assert!((r60 - 53.55).abs() < 0.3, "60 fps rise {r60}");
    assert!((r144 - 55.0).abs() < 0.3, "144 fps rise {r144}");
    assert!(r144 > r60, "higher fps must jump higher");
}

#[test]
fn holding_jump_does_not_rejump_without_auto_jump() {
    let w = flat();
    let cfg = MovementConfig::default();
    let s0 = settled(&w, &cfg, Vec3::ZERO);
    let mut jumps = 0;
    let mut s = s0;
    for _ in 0..240 {
        let (n, r) = step_with_report(
            &s,
            &MovementInput::default().with_buttons(Buttons::JUMP),
            &cfg,
            &w,
            DT,
        );
        jumps += r.jumped as u32;
        s = n;
    }
    assert_eq!(jumps, 1);
}

#[test]
fn bunny_hop_skips_friction_on_jump_tick() {
    let w = flat();
    let cfg = MovementConfig::default();
    let mut s = settled(&w, &cfg, Vec3::ZERO);
    s.velocity = Vec3::new(400.0, 0.0, 0.0);
    // Long on the ground: outside the bhop penalty window.
    s.time_on_ground = 2.0;
    let (after, rep) = step_with_report(
        &s,
        &MovementInput::default().with_buttons(Buttons::JUMP),
        &cfg,
        &w,
        DT,
    );
    assert!(rep.jumped);
    assert_eq!(rep.friction_drop, 0.0);
    assert!((after.horizontal_speed() - 400.0).abs() < 1e-3);
}

#[test]
fn falling_player_lands_and_is_grounded() {
    let w = flat();
    let cfg = MovementConfig::default();
    let s = run(
        &w,
        &cfg,
        MovementState::at(Vec3::new(0.0, 0.0, 300.0)),
        120,
        |_| MovementInput::default(),
    );
    assert!(s.on_ground());
    assert!(
        s.position.z >= 0.0 && s.position.z < 0.1,
        "z {}",
        s.position.z
    );
    assert_eq!(s.velocity.z, 0.0);
}

#[test]
fn free_fall_without_collision_follows_parabola_exactly() {
    let cfg = MovementConfig::default();
    let mut s = MovementState::at(Vec3::new(0.0, 0.0, 1000.0));
    for _ in 0..60 {
        s = step(&s, &MovementInput::default(), &cfg, &EmptyWorld, DT);
    }
    let t = 60.0 * DT;
    let expected = 1000.0 - 0.5 * cfg.gravity * t * t;
    assert!(
        (s.position.z - expected).abs() < 0.05,
        "{} vs {}",
        s.position.z,
        expected
    );
    assert!((s.velocity.z + cfg.gravity * t).abs() < 0.01);
}

#[test]
fn walls_are_never_penetrated() {
    let w = proving_ground();
    let cfg = MovementConfig::default();
    // Sprint straight into the +X perimeter wall at x = 1500.
    let s = run(
        &w,
        &cfg,
        MovementState::at(Vec3::new(1200.0, 0.0, 0.0)),
        300,
        |_| MovementInput::forward().with_buttons(Buttons::SPRINT),
    );
    let front = s.position.x + cfg.stand_hull.maxs.x;
    assert!(front <= 1500.0 && front > 1499.9, "front face at {front}");
    assert!(s.velocity.x.abs() < 1e-3);
}

#[test]
fn sliding_along_a_wall_keeps_tangential_speed() {
    let w = proving_ground();
    let cfg = MovementConfig::default();
    // Run diagonally (+X,+Y) into the +Y perimeter wall at y = 600.
    let s = run(
        &w,
        &cfg,
        MovementState::at(Vec3::new(0.0, 500.0, 0.0)),
        200,
        |_| MovementInput::forward().with_yaw(45.0),
    );
    assert!(s.position.y + cfg.stand_hull.maxs.y <= 600.0);
    assert!(
        s.velocity.x > 100.0,
        "kept sliding along the wall: {:?}",
        s.velocity
    );
}

#[test]
fn walks_up_stairs() {
    let w = proving_ground();
    let cfg = MovementConfig::default();
    // Stairs at x=100..292, y=300..420, 16u per step, top at z=128.
    let mut s = MovementState::at(Vec3::new(40.0, 360.0, 0.0));
    let mut peak = 0.0_f32;
    for _ in 0..180 {
        s = step(&s, &MovementInput::forward(), &cfg, &w, DT);
        peak = peak.max(s.position.z);
        // Every tick on the staircase must be grounded: stepping, not hopping.
        if s.position.x < 270.0 {
            assert!(
                s.on_ground(),
                "airborne on the stairs at x = {}",
                s.position.x
            );
        }
    }
    assert!(peak > 127.0, "climbed to z = {peak}");
}

#[test]
fn walking_into_a_40u_block_mantles_onto_it() {
    let w = proving_ground();
    let cfg = MovementConfig::default();
    // 40u block at x=500: taller than the 18u step height, low enough to
    // mantle from the ground while holding W.
    let mut s = MovementState::at(Vec3::new(440.0, 360.0, 0.0));
    let mut finished = false;
    for _ in 0..120 {
        let (n, rep) = step_with_report(&s, &MovementInput::forward(), &cfg, &w, DT);
        s = n;
        if rep.mantle_finished {
            finished = true;
            break;
        }
    }
    assert!(finished, "mantle should finish");
    assert!(
        (s.position.z - 40.0).abs() < 0.5,
        "on top: z = {}",
        s.position.z
    );
    assert!(s.position.x > 500.0);
}

#[test]
fn a_tall_wall_is_not_mantled_from_the_ground() {
    let w = proving_ground();
    let cfg = MovementConfig::default();
    // 100u block (x=980..1076): above the ground mantle limit.
    let s = run(
        &w,
        &cfg,
        MovementState::at(Vec3::new(920.0, 360.0, 0.0)),
        120,
        |_| MovementInput::forward(),
    );
    assert!(s.position.z < 1.0);
    assert!(s.position.x + cfg.stand_hull.maxs.x <= 980.0);
}

#[test]
fn walks_up_a_30_degree_ramp() {
    let w = proving_ground();
    let cfg = MovementConfig::default();
    // Ramp from (100,-420..-300) rising to z=100 at x≈273.
    let s = run(
        &w,
        &cfg,
        MovementState::at(Vec3::new(40.0, -360.0, 0.0)),
        150,
        |_| MovementInput::forward(),
    );
    assert!(s.position.z > 60.0, "z = {}", s.position.z);
}

#[test]
fn crouch_lowers_speed_and_needs_headroom_to_stand() {
    let mut w = flat();
    // A 60u-high ceiling slab between x=200 and x=400.
    w.add(Brush::cuboid(
        Vec3::new(200.0, -200.0, 60.0),
        Vec3::new(400.0, 200.0, 100.0),
    ));
    let cfg = MovementConfig::default();
    let s0 = settled(&w, &cfg, Vec3::ZERO);
    let crouch = MovementInput::forward().with_buttons(Buttons::CROUCH);
    let mut s = run(&w, &cfg, s0, 200, |_| crouch);
    assert!(s.crouched);
    assert!(s.horizontal_speed() <= cfg.crouch_speed + 1e-3);
    // Stop under the slab and release crouch: must stay crouched.
    s.position.x = 300.0;
    s.velocity = Vec3::ZERO;
    let s = step(&s, &MovementInput::default(), &cfg, &w, DT);
    assert!(s.crouched, "no headroom to stand");
}

#[test]
fn render_frame_rate_does_not_change_simulation() {
    // Same wall-clock span driven at 30 fps vs 144 fps (with jitter) must yield
    // the same tick count and therefore identical state.
    let w = proving_ground();
    let cfg = MovementConfig::default();
    let simulate = |frame_times: &[f64]| {
        let mut clock = FixedClock::new(60.0);
        let mut s = MovementState::at(SPAWN);
        let mut tick = 0usize;
        for &ft in frame_times {
            for _ in 0..clock.advance(ft) {
                let input = MovementInput {
                    forward: 1.0,
                    yaw: tick as f32 * 0.25,
                    buttons: if tick == 50 {
                        Buttons::JUMP
                    } else {
                        Buttons::NONE
                    },
                    ..MovementInput::default()
                };
                s = step(&s, &input, &cfg, &w, clock.dt());
                tick += 1;
            }
        }
        s
    };
    // Both cover 3 s plus half a tick, so float error in the accumulator
    // cannot land on a tick boundary.
    let mut at_30: Vec<f64> = vec![1.0 / 30.0; 90];
    let mut at_144: Vec<f64> = (0..432u32)
        .map(|i| {
            if i.is_multiple_of(2) {
                1.0 / 120.0
            } else {
                1.0 / 180.0
            }
        })
        .collect();
    at_30.push(1.0 / 120.0);
    at_144.push(1.0 / 120.0);
    let a = simulate(&at_30);
    let b = simulate(&at_144);
    assert_eq!(a.tick, b.tick);
    assert_eq!(a, b);
}
