//! Golden tests: numbers measured on the R5Flowstate S21 client
//! (`bridge_move_trace`, 2026-10-02, 60 and 144 fps; see
//! docs/MOVEMENT-MEASUREMENTS.md). If one of these fails after a rules change,
//! the change moved us away from real Apex behaviour.

use apex_collision::{Brush, BrushWorld};
use apex_core::*;

const DT60: f32 = 1.0 / 60.0;
const DT144: f32 = 1.0 / 144.0;

fn flat() -> BrushWorld {
    let mut w = BrushWorld::new();
    w.add(Brush::cuboid(
        Vec3::new(-50000.0, -50000.0, -32.0),
        Vec3::new(50000.0, 50000.0, 0.0),
    ));
    w
}

fn grounded(w: &BrushWorld, cfg: &MovementConfig, velocity: Vec3) -> MovementState {
    let mut s = MovementState::at(Vec3::ZERO);
    for _ in 0..3 {
        s = step(&s, &MovementInput::default(), cfg, w, DT60);
    }
    assert!(s.on_ground());
    s.velocity = velocity;
    s
}

fn input(forward: f32, right: f32, buttons: Buttons) -> MovementInput {
    MovementInput {
        forward,
        right,
        buttons,
        ..MovementInput::default()
    }
}

fn heading(v: Vec3) -> f32 {
    v.y.atan2(v.x).to_degrees()
}

#[test]
fn start_kick_then_run_accel_matches_trace() {
    // Trace (60 fps): 47.92, 95.84, 143.76, 152.39, 161.01, ... 199.525.
    let w = flat();
    let cfg = MovementConfig::default();
    let mut s = grounded(&w, &cfg, Vec3::ZERO);
    let mut speeds = Vec::new();
    for _ in 0..12 {
        s = step(&s, &input(1.0, 0.0, Buttons::NONE), &cfg, &w, DT60);
        speeds.push(s.horizontal_speed());
    }
    let expect = [47.92, 95.84, 143.76, 152.39, 161.01, 169.64, 178.27, 186.89];
    for (i, e) in expect.iter().enumerate() {
        assert!(
            (speeds[i] - e).abs() < 0.1,
            "tick {}: {} vs {e}",
            i + 1,
            speeds[i]
        );
    }
    assert!((speeds[11] - 199.525).abs() < 0.01);
}

#[test]
fn sprint_ramp_is_slow() {
    // Run (199.525) -> sprint (299) at a net 115 u/s^2: ~0.865 s.
    let w = flat();
    let cfg = MovementConfig::default();
    let mut s = grounded(&w, &cfg, Vec3::new(199.525, 0.0, 0.0));
    let sprint = input(1.0, 0.0, Buttons::SPRINT);
    let mut t = 0.0;
    while s.horizontal_speed() < 298.99 {
        s = step(&s, &sprint, &cfg, &w, DT60);
        t += DT60;
        assert!(t < 2.0);
    }
    assert!((t - 0.865).abs() < 0.03, "ramp took {t} s");
}

#[test]
fn friction_is_constant_1250_standing_and_1500_crouched() {
    let w = flat();
    let cfg = MovementConfig::default();
    for (dt, crouch, rate) in [
        (DT60, false, 1250.0),
        (DT144, false, 1250.0),
        (DT60, true, 1500.0),
    ] {
        let mut s = grounded(&w, &cfg, Vec3::new(80.0, 0.0, 0.0));
        let b = if crouch {
            Buttons::CROUCH
        } else {
            Buttons::NONE
        };
        if crouch {
            // Fully crouch first (0.38 s), at rest.
            s.velocity = Vec3::ZERO;
            for _ in 0..40 {
                s = step(&s, &input(0.0, 0.0, b), &cfg, &w, DT60);
            }
            assert!(s.crouched);
            s.velocity = Vec3::new(80.0, 0.0, 0.0);
        }
        let before = s.horizontal_speed();
        s = step(&s, &input(0.0, 0.0, b), &cfg, &w, dt);
        let decel = (before - s.horizontal_speed()) / dt;
        assert!(
            (decel - rate).abs() < 1.0,
            "crouch={crouch} dt={dt}: {decel}"
        );
    }
}

#[test]
fn slide_boost_cap_cooldown_and_end() {
    let w = flat();
    let cfg = MovementConfig::default();
    let sprint = input(1.0, 0.0, Buttons::SPRINT);
    let slide = input(1.0, 0.0, Buttons::SPRINT | Buttons::CROUCH);

    // From 299: boost capped at 460, then one tick of slide decel (trace: 455.6).
    let s = grounded(&w, &cfg, Vec3::new(299.0, 0.0, 0.0));
    let (s, rep) = step_with_report(&s, &slide, &cfg, &w, DT60);
    assert!(rep.slide_started && rep.slide_boosted && s.sliding);
    assert!(
        (s.horizontal_speed() - 455.6).abs() < 0.3,
        "{}",
        s.horizontal_speed()
    );

    // From 220: +174.4 uncapped (trace 219.98 -> 394.24 before decel).
    let s2 = grounded(&w, &cfg, Vec3::new(219.98, 0.0, 0.0));
    let (s2, _) = step_with_report(&s2, &slide, &cfg, &w, DT60);
    let decel = (cfg.slide_decel_const + cfg.slide_decel_linear * 394.38) * DT60;
    assert!((s2.horizontal_speed() - (394.38 - decel)).abs() < 0.3);

    // Slide until it ends below 125, then crouch friction.
    let mut s = s;
    let mut ended_at = None;
    for _ in 0..600 {
        let before = s.horizontal_speed();
        s = step(&s, &slide, &cfg, &w, DT60);
        if !s.sliding {
            ended_at = Some(before);
            break;
        }
    }
    let end = ended_at.expect("slide should end");
    assert!(end < 125.0 && end > 120.0, "ended at {end}");

    // Re-slide right away: no boost (cooldown ~2.4 s).
    let mut s = grounded(&w, &cfg, Vec3::new(299.0, 0.0, 0.0));
    s.time_since_slide_boost = 1.0;
    let (_, rep) = step_with_report(&s, &slide, &cfg, &w, DT60);
    assert!(rep.slide_started && !rep.slide_boosted);
    let _ = sprint;
}

/// Jump along +X at 299 looking down +X; press `right` at the first tick at or
/// after `press_t` seconds since the first airborne tick. Returns the heading
/// change on the press tick and the speed change. The press lands on the tick
/// nearest `press_t`.
fn lurch_at(press_t: f32, dt: f32, hold_from_takeoff: bool) -> (f32, f32, f32) {
    let w = flat();
    let cfg = MovementConfig::default();
    let mut s = grounded(&w, &cfg, Vec3::new(299.0, 0.0, 0.0));
    let held = if hold_from_takeoff { 1.0 } else { 0.0 };
    s = step(&s, &input(0.0, held, Buttons::JUMP), &cfg, &w, dt);
    let mut t_air = 0.0;
    loop {
        let right = if t_air + 0.5 * dt >= press_t || hold_from_takeoff {
            1.0
        } else {
            0.0
        };
        let before = s.velocity;
        let (n, rep) = step_with_report(&s, &input(0.0, right, Buttons::NONE), &cfg, &w, dt);
        s = n;
        if right != 0.0 {
            let dh = (heading(s.velocity) - heading(before)).abs();
            return (dh, s.horizontal_speed() - before.length_2d(), rep.lurch_k);
        }
        t_air += dt;
    }
}

#[test]
fn lurch_matches_trace_profile() {
    // Side presses with W released, mouse still (trace, 60+144 fps):
    // 0.15-0.23 s: 44-45 deg; 0.283: 42.2; 0.317: 33.7; 0.383: 18.5; 0.433: 9.2.
    for (t, game) in [
        (0.151, 45.0),
        (0.233, 43.95),
        (0.283, 42.2),
        (0.317, 33.7),
        (0.383, 18.5),
        (0.433, 9.2),
    ] {
        let (dh, dv, _) = lurch_at(t, DT60, false);
        assert!((dh - game).abs() < 1.5, "t={t}: {dh} vs {game}");
        assert!(dv.abs() < 0.5, "lurch keeps speed: {dv}");
    }
    // Same strength per press at 144 fps (trace 0.236 s: 43.3 deg, 0.431 s: 10.3).
    for (t, game) in [(0.236, 43.3), (0.431, 10.3)] {
        let (dh, _, _) = lurch_at(t, DT144, false);
        assert!((dh - game).abs() < 1.5, "144fps t={t}: {dh} vs {game}");
    }
}

#[test]
fn no_lurch_for_keys_held_at_takeoff_or_after_window() {
    let (_, _, k_held) = lurch_at(0.0, DT60, true);
    assert_eq!(k_held, 0.0);
    let (dh, _, k_late) = lurch_at(0.55, DT60, false);
    assert_eq!(k_late, 0.0);
    // Only plain air control remains: ~1.6 deg/tick at 60 fps (trace 1.60).
    assert!((dh - 1.6).abs() < 0.2, "{dh}");
}

#[test]
fn air_strafe_gain_stops_at_cap() {
    // Hold right from after the window with the camera fixed: gain ~8.25 u/s
    // per 60 fps tick perpendicular until v.wish reaches ~70 (trace 68.9-69.9).
    let w = flat();
    let cfg = MovementConfig::default();
    let mut s = grounded(&w, &cfg, Vec3::new(299.0, 0.0, 0.0));
    s = step(&s, &input(0.0, 0.0, Buttons::JUMP), &cfg, &w, DT60);
    for _ in 0..34 {
        s = step(&s, &input(0.0, 0.0, Buttons::NONE), &cfg, &w, DT60);
    }
    let right_dir = Vec3::new(0.0, -1.0, 0.0); // yaw 0: right is -Y
    for _ in 0..12 {
        s = step(&s, &input(0.0, 1.0, Buttons::NONE), &cfg, &w, DT60);
    }
    let proj = s.velocity.horizontal().dot(right_dir);
    assert!(
        s.on_ground() || (proj - 70.0).abs() < 1.0,
        "projection {proj}"
    );
}

#[test]
fn crouch_takes_time_on_ground() {
    let w = flat();
    let cfg = MovementConfig::default();
    let mut s = grounded(&w, &cfg, Vec3::ZERO);
    let mut ticks = 0;
    while !s.crouched {
        s = step(&s, &input(0.0, 0.0, Buttons::CROUCH), &cfg, &w, DT60);
        ticks += 1;
        assert!(ticks < 100);
    }
    // Trace: duck state 1 -> 2 in 23 ticks (0.383 s).
    assert!((22..=24).contains(&ticks), "{ticks}");
}

/// Jump, land, wait `ground_wait` seconds on the ground, jump again; returns
/// the second jump's launch speed V (= first-tick v_z + 1.5 g dt).
fn second_jump_launch(ground_wait: f32, crouch: bool, sliding_speed: Option<f32>) -> f32 {
    let w = flat();
    let cfg = MovementConfig::default();
    let mut s = grounded(&w, &cfg, Vec3::new(sliding_speed.unwrap_or(0.0), 0.0, 0.0));
    let c = if crouch {
        Buttons::CROUCH
    } else {
        Buttons::NONE
    };
    let fwd = if sliding_speed.is_some() { 1.0 } else { 0.0 };
    s = step(&s, &input(fwd, 0.0, Buttons::JUMP | c), &cfg, &w, DT60);
    while !s.on_ground() {
        s = step(&s, &input(fwd, 0.0, c), &cfg, &w, DT60);
    }
    let mut waited = 0.0;
    while waited + 0.5 * DT60 < ground_wait {
        s = step(&s, &input(fwd, 0.0, c), &cfg, &w, DT60);
        waited += DT60;
    }
    let s = step(&s, &input(fwd, 0.0, Buttons::JUMP | c), &cfg, &w, DT60);
    s.velocity.z + 1.5 * cfg.gravity * DT60
}

#[test]
fn jump_fatigue_matches_trace() {
    // Session 4: jump within 0.14 s of a jump-landing -> 158.7 (30 % height);
    // 0.45 s -> ~236; 0.55 s -> ~257; >= 0.735 s -> 289.8.
    for (wait, game) in [
        (0.0, 158.7),
        (0.1, 158.7),
        (0.45, 236.4),
        (0.55, 257.3),
        (1.0, 289.8),
    ] {
        let v = second_jump_launch(wait, false, None);
        assert!((v - game).abs() < 4.0, "ground {wait}s: {v} vs {game}");
    }
    // Holding crouch does not change it (test 12 == test 11).
    let v = second_jump_launch(0.0, true, None);
    assert!((v - 158.7).abs() < 1.5, "crouch-held spam {v}");
}

#[test]
fn slide_jump_is_0944_and_reslides_on_landing() {
    // Sprint-crouch spam: fresh slide-jump 273.5, chained 150.0 (test 13).
    let w = flat();
    let cfg = MovementConfig::default();
    let mut s = grounded(&w, &cfg, Vec3::new(299.0, 0.0, 0.0));
    let slide = input(1.0, 0.0, Buttons::SPRINT | Buttons::CROUCH);
    s = step(&s, &slide, &cfg, &w, DT60);
    assert!(s.sliding);
    let jump = input(1.0, 0.0, Buttons::SPRINT | Buttons::CROUCH | Buttons::JUMP);
    s = step(&s, &jump, &cfg, &w, DT60);
    let v_fresh = s.velocity.z + 1.5 * cfg.gravity * DT60;
    assert!((v_fresh - 273.5).abs() < 1.0, "fresh slide-jump {v_fresh}");
    while !s.on_ground() {
        s = step(&s, &slide, &cfg, &w, DT60);
    }
    // Landing tick re-entered the slide, so an immediate jump is a slide-jump.
    let s = step(&s, &jump, &cfg, &w, DT60);
    let v_chain = s.velocity.z + 1.5 * cfg.gravity * DT60;
    assert!(
        (v_chain - 150.0).abs() < 1.0,
        "chained slide-jump {v_chain}"
    );
}

#[test]
fn walking_off_a_ledge_does_not_fatigue() {
    let cfg = MovementConfig::default();
    let mut w = flat();
    w.add(Brush::cuboid(
        Vec3::new(-100.0, -100.0, 0.0),
        Vec3::new(10.0, 100.0, 40.0),
    ));
    let mut s = MovementState::at(Vec3::new(0.0, 0.0, 40.0));
    s = step(&s, &MovementInput::default(), &cfg, &w, DT60);
    for _ in 0..120 {
        s = step(&s, &input(1.0, 0.0, Buttons::NONE), &cfg, &w, DT60);
        if s.on_ground() && s.position.z < 1.0 {
            break;
        }
    }
    assert!(s.on_ground() && s.position.z < 1.0);
    let s = step(&s, &input(1.0, 0.0, Buttons::JUMP), &cfg, &w, DT60);
    let v = s.velocity.z + 1.5 * cfg.gravity * DT60;
    assert!((v - 289.8).abs() < 0.5, "{v}");
}
