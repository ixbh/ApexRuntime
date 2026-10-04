//! Golden tests for traversal, against R5Flowstate S21 measurements
//! (sessions 4-5, 2026-10-03; docs/MOVEMENT-MEASUREMENTS.md).

use apex_collision::{Brush, BrushWorld};
use apex_core::*;

const DT60: f32 = 1.0 / 60.0;
const DT165: f32 = 1.0 / 165.0;

/// Flat floor plus a block in front (+X) whose near face is at `face_x`.
fn world_with_block(face_x: f32, height: f32) -> BrushWorld {
    let mut w = BrushWorld::new();
    w.add(Brush::cuboid(
        Vec3::new(-5000.0, -5000.0, -32.0),
        Vec3::new(5000.0, 5000.0, 0.0),
    ));
    w.add(Brush::cuboid(
        Vec3::new(face_x, -2000.0, 0.0),
        Vec3::new(face_x + 400.0, 2000.0, height),
    ));
    w
}

fn input(forward: f32, right: f32, buttons: Buttons) -> MovementInput {
    MovementInput {
        forward,
        right,
        buttons,
        ..MovementInput::default()
    }
}

fn grounded_at(w: &BrushWorld, cfg: &MovementConfig, x: f32, vx: f32) -> MovementState {
    let mut s = MovementState::at(Vec3::new(x, 0.0, 0.0));
    settle(&mut s, cfg, w);
    assert!(s.on_ground());
    s.velocity = Vec3::new(vx, 0.0, 0.0);
    s
}

const SPRINT_W: Buttons = Buttons::SPRINT;

/// Sprint-jump into a tall wall holding W; returns per-tick (t since takeoff, state, report).
fn climb_run(dt: f32, ticks: usize) -> Vec<(f32, MovementState, StepReport)> {
    let w = world_with_block(45.0, 2000.0);
    let cfg = MovementConfig::default();
    let mut s = grounded_at(&w, &cfg, 0.0, 299.0);
    let mut out = Vec::new();
    let (n, r) = step_with_report(&s, &input(1.0, 0.0, SPRINT_W | Buttons::JUMP), &cfg, &w, dt);
    s = n;
    out.push((0.0, s, r));
    for i in 1..ticks {
        let (n, r) = step_with_report(&s, &input(1.0, 0.0, SPRINT_W), &cfg, &w, dt);
        s = n;
        out.push((i as f32 * dt, s, r));
    }
    out
}

#[test]
fn wall_climb_matches_trace() {
    // 13 climbs: attach 0.267 s after takeoff, peak 182.5 at +0.184 s,
    // climb 0.833 s, rise to the end of the climb 142.8 u, end push
    // vz 255.5 / |h| 61.7.
    let run = climb_run(DT60, 200);
    let attach = run
        .iter()
        .find(|(_, _, r)| r.climb_started)
        .expect("climb starts");
    assert!((attach.0 - 0.267).abs() < 0.03, "attach at {}", attach.0);
    let peak = run
        .iter()
        .filter(|(_, s, _)| s.climb.is_some())
        .find(|(_, s, _)| s.velocity.z >= 182.4)
        .expect("reaches peak");
    assert!(
        (peak.0 - attach.0 - 0.184).abs() < 0.03,
        "peak after {}",
        peak.0 - attach.0
    );
    let end = run
        .iter()
        .find(|(_, _, r)| r.climb_finished)
        .expect("climb ends");
    // S21 tick sequence at 60 fps: attach tick (half gravity), 51 climb ticks,
    // push on the next tick = 52 ticks from attach to push (0.867 s). Replays
    // of 3 recorded climbs match every tick (research/analysis/climb_check.py).
    assert!(
        (end.0 - attach.0 - 0.867).abs() < 0.01,
        "climb lasted {}",
        end.0 - attach.0
    );
    assert!((end.1.velocity.z - 255.5).abs() < 0.1);
    assert!((end.1.velocity.horizontal().length() - 61.7).abs() < 0.1);
    assert!(end.1.velocity.x < 0.0, "pushed away from the wall");
    assert!(
        (end.1.position.z - 142.8).abs() < 5.0,
        "rise {}",
        end.1.position.z
    );
}

#[test]
fn climb_space_drops_after_the_end_push() {
    // Wiki: the end push is a non-jump detach (climb space -128). Any later
    // attach this airtime must be below the lowered cutoff (147 - 128 = 19)
    // and below the first attach point.
    let run = climb_run(DT60, 300);
    let ends: Vec<_> = run.iter().filter(|(_, _, r)| r.climb_finished).collect();
    assert_eq!(ends.len(), 1);
    assert!(
        (ends[0].1.climb_base + 128.0).abs() < 0.5,
        "base {}",
        ends[0].1.climb_base
    );
    for (_, s, r) in run.iter().filter(|(_, _, r)| r.climb_started).skip(1) {
        assert!(
            s.position.z < 19.5,
            "re-attached at {} ({r:?})",
            s.position.z
        );
    }
}

fn wall_jump_at(climb_t: f32, right: f32) -> Vec3 {
    let w = world_with_block(45.0, 2000.0);
    let cfg = MovementConfig::default();
    let mut s = grounded_at(&w, &cfg, 0.0, 299.0);
    s = step(
        &s,
        &input(1.0, 0.0, SPRINT_W | Buttons::JUMP),
        &cfg,
        &w,
        DT60,
    );
    for _ in 0..200 {
        let (n, r) = step_with_report(&s, &input(1.0, 0.0, SPRINT_W), &cfg, &w, DT60);
        s = n;
        if r.climb_started {
            break;
        }
    }
    let mut t = 0.0;
    while t + 0.5 * DT60 < climb_t {
        s = step(&s, &input(1.0, right, SPRINT_W), &cfg, &w, DT60);
        t += DT60;
    }
    let (s, r) = step_with_report(
        &s,
        &input(1.0, right, SPRINT_W | Buttons::JUMP),
        &cfg,
        &w,
        DT60,
    );
    assert!(r.wall_jump);
    s.velocity
}

#[test]
fn wall_jump_mid_climb_keeps_climb_speed() {
    // Session 4 test 5: |h| 258.0 straight out, vz = current climb speed (86-109).
    let v = wall_jump_at(0.4, 0.0);
    assert!((v.horizontal().length() - 258.0).abs() < 0.5);
    assert!(v.z > 80.0 && v.z < 140.0, "vz {}", v.z);
}

/// On the wall at `z` above the baseline moving up at `vz`, jump holding
/// `right` (yaw 0: the wall is ahead along +X).
fn wall_jump_from_zone(z: f32, vz: f32, right: f32) -> (MovementState, StepReport) {
    let w = world_with_block(45.0, 2000.0);
    let cfg = MovementConfig::default();
    let mut s = MovementState::at(Vec3::new(45.0 - 16.0 - 1.0 / 32.0, 0.0, z));
    s.climb_base = 0.0;
    s.velocity = Vec3::new(0.0, 0.0, vz);
    s.climb = Some(Climb {
        normal: Vec3::new(-1.0, 0.0, 0.0),
        t: 0.05,
        peaked: false,
        peak_t: 0.0,
        attach_z: z,
    });
    let (n, r) = step_with_report(&s, &input(0.0, right, Buttons::JUMP), &cfg, &w, DT60);
    assert!(r.wall_jump, "{r:?}");
    (n, r)
}

#[test]
fn wall_bounce_matches_trace() {
    // 9 S21 green-zone bounces (A held): vz follows the height above the
    // baseline, not wall-contact time: 380.4 @24.7, 371.3 @28.0, 339.8
    // @34.4, 260.4 @44.8; |h| 354-365 (258·√2). Climb space -256.
    for (z, vin, want) in [
        (24.7, 21.8, 380.4),
        (28.0, 6.1, 371.3),
        (34.4, 56.0, 339.8),
        (44.8, 35.4, 260.4),
    ] {
        let (s, _) = wall_jump_from_zone(z, vin, 1.0);
        assert!(
            (s.velocity.z - want).abs() < 10.0,
            "z {z}: vz {}",
            s.velocity.z
        );
        assert!((s.velocity.horizontal().length() - 364.9).abs() < 1.0);
        assert!((s.climb_base + 256.0).abs() < 1e-3);
    }
}

#[test]
fn mini_and_neutral_zone_wall_jumps() {
    // Mini zone (wiki): +28.21 u hop, 188 out, climb space -128.
    let (s, _) = wall_jump_from_zone(10.0, 0.0, 0.0);
    assert!(
        (s.velocity.z - (2.0f32 * 750.0 * 28.21).sqrt()).abs() < 1.0,
        "vz {}",
        s.velocity.z
    );
    assert!((s.velocity.horizontal().length() - 188.0).abs() < 0.5);
    assert!((s.climb_base + 128.0).abs() < 1e-3);
    // Neutral zone: no height gain (S21: 122.4 -> 107.3 at 60 fps), 258 out.
    let (s, _) = wall_jump_from_zone(100.0, 122.4, 0.0);
    assert!((s.velocity.z - 109.9).abs() < 0.5, "vz {}", s.velocity.z);
    assert!((s.velocity.horizontal().length() - 258.0).abs() < 0.5);
}

/// Walk into a 48 u ledge holding W (sprint), returning the mantle duration
/// and the state right after it finishes.
fn mantle_48(dt: f32, hold_w_at_end: bool) -> (f32, MovementState) {
    let w = world_with_block(40.0, 48.0);
    let cfg = MovementConfig::default();
    let mut s = grounded_at(&w, &cfg, 0.0, 0.0);
    let mut started = None;
    let mut t = 0.0;
    for _ in 0..1000 {
        let fwd = if started.is_some() && !hold_w_at_end {
            0.0
        } else {
            1.0
        };
        let (n, r) = step_with_report(&s, &input(fwd, 0.0, SPRINT_W), &cfg, &w, dt);
        s = n;
        t += dt;
        if r.mantle_started {
            started = Some(t);
        }
        if r.mantle_finished {
            return (t - started.unwrap(), s);
        }
    }
    panic!("no mantle");
}

#[test]
fn mantle_duration_and_exit_match_trace() {
    // 39 mantles: duration 0.162 + 0.00659·height (48 u -> 0.478 s), forward
    // ~19 u, exit at full speed with W held, 0 without.
    let (d, s) = mantle_48(DT60, true);
    assert!((d - 0.478).abs() < 0.02, "duration {d}");
    assert!((s.position.z - 48.0).abs() < 0.1);
    assert!(
        (s.horizontal_speed() - 299.0).abs() < 0.5,
        "exit {}",
        s.horizontal_speed()
    );
    let (_, s) = mantle_48(DT60, false);
    assert_eq!(s.horizontal_speed(), 0.0);
}

/// Mantle a 48 u ledge at 165 fps, pressing jump `ticks_before_end` ticks
/// before the mantle would finish, then crouch on the next tick if `crouch`.
fn late_jump(ticks_before_end: usize, crouch: bool) -> (Vec3, StepReport) {
    let w = world_with_block(40.0, 48.0);
    let cfg = MovementConfig::default();
    let mut s = grounded_at(&w, &cfg, 0.0, 0.0);
    for _ in 0..1000 {
        let (n, r) = step_with_report(&s, &input(1.0, 0.0, SPRINT_W), &cfg, &w, DT165);
        s = n;
        if r.mantle_started {
            break;
        }
    }
    let m = s.mantle.expect("mantling");
    let ticks_total = ((m.duration - m.t) / DT165).ceil() as usize;
    for _ in 0..ticks_total.saturating_sub(ticks_before_end) {
        s = step(&s, &input(1.0, 0.0, SPRINT_W), &cfg, &w, DT165);
    }
    let (n, r) = step_with_report(
        &s,
        &input(1.0, 0.0, SPRINT_W | Buttons::JUMP),
        &cfg,
        &w,
        DT165,
    );
    s = n;
    if r.jumped {
        return (s.velocity, r);
    }
    let b = if crouch {
        SPRINT_W | Buttons::JUMP | Buttons::CROUCH
    } else {
        SPRINT_W | Buttons::JUMP
    };
    let (n, r) = step_with_report(&s, &input(1.0, 0.0, b), &cfg, &w, DT165);
    (n.velocity, r)
}

#[test]
fn superglide_matches_trace() {
    // 7 superglides at 165 fps: |h| 460.0, vz = a fresh jump (287.6 first tick).
    let (v, r) = late_jump(2, true);
    assert!(r.superglide, "{r:?}");
    assert!((v.horizontal().length() - 460.0).abs() < 0.1);
    assert!((v.z - 287.5).abs() < 1.0, "vz {}", v.z);
}

#[test]
fn mantle_boost_matches_trace() {
    // S21 mantle boost: |h| 388.7 (= 1.3 × 299), 90 u jump (vz 367.4).
    let (v, r) = late_jump(2, false);
    assert!(r.mantle_boost, "{r:?}");
    assert!((v.horizontal().length() - 388.7).abs() < 0.1);
    assert!((v.z - 365.1).abs() < 1.5, "vz {}", v.z);
}

#[test]
fn early_jump_during_mantle_jumps_off() {
    // In the grab (before the last 0.3 s pull): a plain jump off the wall,
    // no horizontal speed (S21: vz 287.6 @165). 48 u mantle = 0.478 s.
    let (v, r) = late_jump((0.4 / DT165) as usize, false);
    assert!(r.jumped && !r.superglide && !r.mantle_boost);
    assert!(v.horizontal().length() < 0.01);
    assert!((v.z - 287.5).abs() < 1.0);
}

#[test]
fn jump_in_the_pull_before_the_window_does_nothing() {
    // Wiki: "if you jump too early you simply won't jump"; the mantle goes on
    // and finishes normally, so scroll-spamming into a ledge still mantles.
    let (_, r) = late_jump((0.25 / DT165) as usize, false);
    assert!(!r.jumped && !r.superglide && !r.mantle_boost, "{r:?}");
    // A 40 u ledge is pull-only (wiki: up to at least 42 u from the ground):
    // scroll-like spam (jump every other tick) from the mantle start never
    // knocks you off; it ends in a normal finish or a late-window boost.
    let w = world_with_block(40.0, 40.0);
    let cfg = MovementConfig::default();
    let mut s = grounded_at(&w, &cfg, 0.0, 0.0);
    let mut mantling = false;
    let mut finished = false;
    for i in 0..400 {
        let spam = mantling && i % 2 == 0;
        let b = if spam {
            SPRINT_W | Buttons::JUMP
        } else {
            SPRINT_W
        };
        let (n, r) = step_with_report(&s, &input(1.0, 0.0, b), &cfg, &w, DT165);
        s = n;
        mantling |= r.mantle_started;
        if mantling && (r.mantle_finished || r.mantle_boost || r.superglide) {
            finished = true;
            break;
        }
        assert!(!(mantling && r.jumped), "knocked off at tick {i}");
    }
    assert!(finished);
}

#[test]
fn superglide_needs_crouch_exactly_one_tick_after_jump() {
    // Same-tick jump+crouch is not a superglide (trainer: "1 frame later").
    let w = world_with_block(40.0, 48.0);
    let cfg = MovementConfig::default();
    let mut s = grounded_at(&w, &cfg, 0.0, 0.0);
    for _ in 0..1000 {
        let (n, r) = step_with_report(&s, &input(1.0, 0.0, SPRINT_W), &cfg, &w, DT165);
        s = n;
        if r.mantle_started {
            break;
        }
    }
    let m = s.mantle.unwrap();
    let ticks_total = ((m.duration - m.t) / DT165).ceil() as usize;
    for _ in 0..ticks_total - 3 {
        s = step(&s, &input(1.0, 0.0, SPRINT_W), &cfg, &w, DT165);
    }
    let both = SPRINT_W | Buttons::JUMP | Buttons::CROUCH;
    let (s, r1) = step_with_report(&s, &input(1.0, 0.0, both), &cfg, &w, DT165);
    let (_, r2) = step_with_report(&s, &input(1.0, 0.0, both), &cfg, &w, DT165);
    assert!(!r1.superglide && !r2.superglide);
}

#[test]
fn superglide_window_is_last_0_15_s() {
    // 0.12 s before the end (inside): superglide. 0.2 s before (outside, in
    // the pull): nothing happens.
    let inside = (0.12 / DT165) as usize;
    let (_, r) = late_jump(inside, true);
    assert!(r.superglide, "{r:?}");
    let outside = (0.2 / DT165) as usize;
    let (_, r) = late_jump(outside, true);
    assert!(!r.superglide && !r.jumped, "{r:?}");
}

#[test]
fn interrupted_mantle_never_leaves_the_hull_inside_the_ledge() {
    // The mantle arc may overlap the ledge edge (as Apex's animation-driven
    // traversal does), but a jump-off at any point must leave a free position.
    let w = world_with_block(40.0, 48.0);
    let cfg = MovementConfig::default();
    for jump_tick in 0..40 {
        let mut s = grounded_at(&w, &cfg, 0.0, 0.0);
        let mut ticks = 0;
        let mut started = false;
        for _ in 0..400 {
            let b = if started && ticks == jump_tick {
                SPRINT_W | Buttons::JUMP
            } else {
                SPRINT_W
            };
            let (n, r) = step_with_report(&s, &input(1.0, 0.0, b), &cfg, &w, DT60);
            s = n;
            if started {
                ticks += 1;
                if s.mantle.is_none() {
                    assert!(
                        w.hull_fits(s.position, cfg.hull(s.crouched)),
                        "inside the ledge after leaving the mantle (jump at {jump_tick})"
                    );
                }
            }
            if r.mantle_started {
                started = true;
            }
            if started && ticks > jump_tick + 5 {
                break;
            }
        }
    }
}

#[test]
fn a_hull_started_inside_solid_is_pushed_out() {
    let w = world_with_block(40.0, 48.0);
    let cfg = MovementConfig::default();
    let s = MovementState::at(Vec3::new(60.0, 0.0, 20.0)); // inside the block
    let (s, r) = step_with_report(&s, &MovementInput::default(), &cfg, &w, DT60);
    assert!(r.unstuck);
    assert!(w.hull_fits(s.position, cfg.hull(s.crouched)));
}

#[test]
fn mantle_without_w_carries_sideways_momentum() {
    // Moving diagonally into a ledge holding only D: the mantle still happens
    // and the sideways speed along the wall is carried through it.
    let w = world_with_block(40.0, 48.0);
    let cfg = MovementConfig::default();
    let mut s = grounded_at(&w, &cfg, 0.0, 0.0);
    // Camera at yaw 45 (looking +X+Y at the wall on +X): D (right) points
    // +X-Y, so it pushes into the wall and along it. Moving that way.
    s.velocity = Vec3::new(150.0, -150.0, 0.0);
    let d_only = MovementInput {
        right: 1.0,
        yaw: 45.0,
        ..MovementInput::default()
    };
    let start_y = s.position.y;
    let mut started = false;
    for _ in 0..200 {
        let (n, r) = step_with_report(&s, &d_only, &cfg, &w, DT60);
        s = n;
        started |= r.mantle_started;
        if r.mantle_finished {
            break;
        }
    }
    assert!(started, "D toward the wall is enough to mantle");
    assert!(
        (s.position.z - 48.0).abs() < 0.5,
        "on top: {}",
        s.position.z
    );
    assert!(
        start_y - s.position.y > 40.0,
        "moved sideways during the mantle: {}",
        start_y - s.position.y
    );
    assert!(
        s.velocity.y < -100.0,
        "sideways speed kept: {:?}",
        s.velocity
    );
}

#[test]
fn wall_contact_without_w_allows_a_bounce() {
    // Recorded bounces attached with only A held (no W).
    let w = world_with_block(45.0, 2000.0);
    let cfg = MovementConfig::default();
    let mut s = grounded_at(&w, &cfg, 0.0, 299.0);
    s = step(
        &s,
        &input(1.0, 0.0, SPRINT_W | Buttons::JUMP),
        &cfg,
        &w,
        DT60,
    );
    // Camera at yaw 45: D pushes into the wall (+X) and along it.
    let d_only = MovementInput {
        right: 1.0,
        yaw: 45.0,
        ..MovementInput::default()
    };
    let mut contact = false;
    for _ in 0..60 {
        let (n, r) = step_with_report(&s, &d_only, &cfg, &w, DT60);
        s = n;
        if r.climb_started {
            contact = true;
            break;
        }
    }
    assert!(contact, "wall contact with A/D only");
    let jump = MovementInput {
        buttons: Buttons::JUMP,
        ..d_only
    };
    let (s, r) = step_with_report(&s, &jump, &cfg, &w, DT60);
    assert!(r.wall_jump);
    assert!((s.velocity.horizontal().length() - 364.9).abs() < 1.0);
}

#[test]
fn sideways_climb_is_a_wallrun() {
    // Wiki "Sideways climb - Wallrun": A/D pointing slightly into the wall
    // (view turned a little away) climbs sideways up to 258 u/s, stays on
    // the wall for the 1 s non-upward timer, then lets go.
    let w = world_with_block(45.0, 2000.0);
    let cfg = MovementConfig::default();
    let mut s = grounded_at(&w, &cfg, 0.0, 299.0);
    s = step(
        &s,
        &input(1.0, 0.0, SPRINT_W | Buttons::JUMP),
        &cfg,
        &w,
        DT60,
    );
    let d_run = MovementInput {
        right: 1.0,
        yaw: 10.0,
        ..MovementInput::default()
    };
    let mut attach = None;
    let mut max_along = 0.0f32;
    let mut z_range = (f32::MAX, f32::MIN);
    for i in 0..150 {
        let (n, r) = step_with_report(&s, &d_run, &cfg, &w, DT60);
        s = n;
        if r.climb_started {
            attach = Some(i);
        }
        assert!(!r.mantle_started);
        if attach.is_some() && s.climb.is_some() {
            max_along = max_along.max(s.velocity.horizontal().length());
            z_range = (z_range.0.min(s.position.z), z_range.1.max(s.position.z));
        }
        if let Some(a) = attach
            && s.climb.is_none()
        {
            let on_wall = (i - a) as f32 * DT60;
            assert!((on_wall - 1.0).abs() < 0.05, "on the wall for {on_wall}");
            assert!((max_along - 258.0).abs() < 1.0, "along {max_along}");
            assert!(z_range.1 - z_range.0 < 15.0, "height range {z_range:?}");
            assert!(
                s.position.x > 10.0,
                "held against the wall: x {}",
                s.position.x
            );
            return;
        }
    }
    panic!("no wallrun (attach {attach:?})");
}

#[test]
fn looking_away_or_crouching_detaches_a_climb() {
    let run = climb_run(DT60, 40);
    let (_, s, _) = run
        .iter()
        .find(|(_, s, _)| s.climb.is_some())
        .copied()
        .expect("climbing");
    let w = world_with_block(45.0, 2000.0);
    let cfg = MovementConfig::default();
    let look_away = MovementInput {
        forward: 1.0,
        yaw: 50.0,
        ..MovementInput::default()
    };
    assert!(step(&s, &look_away, &cfg, &w, DT60).climb.is_none());
    let crouch = input(1.0, 0.0, Buttons::CROUCH);
    assert!(step(&s, &crouch, &cfg, &w, DT60).climb.is_none());
}

/// Sprint into a 48 u ledge at 165 fps holding W, then on the superglide
/// window press jump (and crouch one tick later if `crouch`) with `exit`
/// as the movement input. Returns the state just after the exit.
fn superglide_with(exit: MovementInput, crouch: bool) -> (MovementState, StepReport) {
    let w = world_with_block(40.0, 48.0);
    let cfg = MovementConfig::default();
    let mut s = grounded_at(&w, &cfg, 0.0, 0.0);
    for _ in 0..1000 {
        let (n, r) = step_with_report(&s, &input(1.0, 0.0, SPRINT_W), &cfg, &w, DT165);
        s = n;
        if r.mantle_started {
            break;
        }
    }
    let m = s.mantle.expect("mantling");
    let ticks_total = ((m.duration - m.t) / DT165).ceil() as usize;
    for _ in 0..ticks_total - 3 {
        s = step(&s, &input(1.0, 0.0, SPRINT_W), &cfg, &w, DT165);
    }
    let jump = MovementInput {
        buttons: exit.buttons | Buttons::JUMP,
        ..exit
    };
    s = step(&s, &jump, &cfg, &w, DT165);
    let second = MovementInput {
        buttons: if crouch {
            jump.buttons | Buttons::CROUCH
        } else {
            jump.buttons
        },
        ..exit
    };
    step_with_report(&s, &second, &cfg, &w, DT165)
}

#[test]
fn tap_strafe_out_of_a_superglide() {
    // Wiki: a superglide uses a jump input, so it starts a lurch window and
    // can be tap-strafed. The old clock (time since leaving the ground) had
    // run out during the mantle.
    let (mut s, r) = superglide_with(input(1.0, 0.0, SPRINT_W), true);
    assert!(r.superglide, "{r:?}");
    let w = world_with_block(40.0, 48.0);
    let cfg = MovementConfig::default();
    let glide = s.velocity.horizontal();
    for _ in 0..10 {
        s = step(&s, &input(0.0, 0.0, Buttons::NONE), &cfg, &w, DT165);
    }
    let (n, r) = step_with_report(&s, &input(0.0, 1.0, Buttons::NONE), &cfg, &w, DT165);
    assert!(r.lurch_k > 0.4, "lurch k {}", r.lurch_k);
    let turned = glide
        .normalize_or_zero()
        .dot(n.velocity.horizontal().normalize_or_zero());
    assert!(turned < 0.9, "heading barely changed");
    assert!(
        (n.horizontal_speed() - glide.length()).abs() < 1.0,
        "speed kept"
    );
}

#[test]
fn mantle_boost_disables_lurch_until_landing() {
    // Wiki + S21 (an A press 0.25 s after a boost turned 0.0°).
    let (mut s, r) = superglide_with(input(1.0, 0.0, SPRINT_W), false);
    assert!(r.mantle_boost, "{r:?}");
    let w = world_with_block(40.0, 48.0);
    let cfg = MovementConfig::default();
    for _ in 0..10 {
        s = step(&s, &input(0.0, 0.0, Buttons::NONE), &cfg, &w, DT165);
    }
    let (_, r) = step_with_report(&s, &input(0.0, 1.0, Buttons::NONE), &cfg, &w, DT165);
    assert_eq!(r.lurch_k, 0.0);
}

#[test]
fn sideways_superglide_follows_the_input() {
    // Wiki: the pull finishes without W, so switching to A/D before the
    // superglide inputs sends it sideways (at full sprint-glide speed).
    let d = MovementInput {
        right: 1.0,
        buttons: SPRINT_W,
        ..MovementInput::default()
    };
    let (s, r) = superglide_with(d, true);
    assert!(r.superglide, "{r:?}");
    let h = s.velocity.horizontal();
    assert!((h.length() - 460.0).abs() < 0.5, "|h| {}", h.length());
    assert!(h.y.abs() > 0.9 * h.length(), "sideways: {h:?}");
}

#[test]
fn no_ma_fatigue_bounce_on_a_chest_high_wall() {
    // Wiki "No-ma-Bounce": W+D with the view turned 5°+ toward D points the
    // input more than 50° off the wall, so a fatigued jump into a 48 u wall
    // climbs instead of mantling, hangs at the ledge, and jumps off as a
    // wall bounce. Holding W straight at it mantles.
    let w = world_with_block(45.0, 48.0);
    let cfg = MovementConfig::default();
    let mut s = grounded_at(&w, &cfg, 45.0 - 16.0 - 1.0 / 32.0 - 4.0, 0.0);
    s.landed_from_jump = true;
    s.time_on_ground = 0.0;
    let no_ma = MovementInput {
        forward: 1.0,
        right: 1.0,
        yaw: -10.0,
        ..MovementInput::default()
    };
    let jump = MovementInput {
        buttons: Buttons::JUMP,
        ..no_ma
    };
    s = step(&s, &jump, &cfg, &w, DT60);
    let mut climbed = false;
    for _ in 0..40 {
        let (n, r) = step_with_report(&s, &no_ma, &cfg, &w, DT60);
        s = n;
        assert!(!r.mantle_started, "mantled at z {}", s.position.z);
        climbed |= r.climb_started;
    }
    assert!(climbed && s.climb.is_some(), "hanging on the wall");
    assert!(s.position.z < 48.0, "below the ledge: {}", s.position.z);
    let (s, r) = step_with_report(&s, &jump, &cfg, &w, DT60);
    assert!(r.wall_jump);
    // Mini or green zone depending on where the no-mantle hang holds you:
    // OPEN, to be measured in Flowstate (session 6, No-ma bounce on 48 u).
    assert!(s.velocity.z > 200.0, "bounce vz {}", s.velocity.z);

    // Same jump looking straight at it with W: mantles.
    let mut s = grounded_at(&w, &cfg, 45.0 - 16.0 - 1.0 / 32.0 - 4.0, 0.0);
    s.landed_from_jump = true;
    s.time_on_ground = 0.0;
    let w_only = input(1.0, 0.0, Buttons::NONE);
    s = step(&s, &input(1.0, 0.0, Buttons::JUMP), &cfg, &w, DT60);
    let mut mantled = false;
    for _ in 0..40 {
        let (n, r) = step_with_report(&s, &w_only, &cfg, &w, DT60);
        s = n;
        mantled |= r.mantle_started;
    }
    assert!(mantled);
}
