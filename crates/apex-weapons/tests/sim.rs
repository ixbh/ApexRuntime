use apex_weapons::*;

fn gun(fire_mode: FireMode, charge: Charge) -> GunSpec {
    GunSpec {
        id: "test".into(),
        fire_mode,
        charge,
        fire_interval: 1.0 / 18.0,
        rechamber: 0.0,
        clip: 17,
        ammo_per_shot: 1,
        pellets: 1,
        blast: Vec::new(),
        reload: 1.8,
        reload_add: 1.0,
        reload_empty: 2.45,
        reload_empty_add: 1.0,
        // Measured R-99 points (Flowstate): ammo in at 80%, ready at 90%,
        // one milestone; the resumed reload is shorter.
        reloads: Reloads {
            tactical: ReloadPlan { ammo_in: 1.44, ready: 1.61, milestones: vec![0.34] },
            empty: ReloadPlan { ammo_in: 1.96, ready: 2.2, milestones: vec![0.36] },
            resume: vec![ReloadPlan { ammo_in: 0.8, ready: 1.05, milestones: Vec::new() }],
            empty_resume: vec![ReloadPlan { ammo_in: 1.0, ready: 1.2, milestones: Vec::new() }],
        },
        raise: 0.3,
        holster: 0.3,
        sprint_out: 0.24,
        inspect: 4.0,
        zoom_in: 0.2,
        zoom_out: 0.2,
        zoom_fov: 60.0,
        ammo: None,
        ads_move_scale: 1.0,
        spread: Spread {
            stand_hip: 2.0,
            run_hip: 3.0,
            sprint_hip: 5.0,
            crouch_hip: 1.6,
            air_hip: 7.0,
            stand_ads: 0.35,
            crouch_ads: 0.25,
            air_ads: 5.0,
            kick_hip: 0.18,
            max_kick: 2.0,
            decay_delay: 0.15,
            decay_rate: 12.0,
        },
        damage: Damage {
            near: 11.0,
            far: 11.0,
            very_far: 11.0,
            near_dist: 900.0,
            far_dist: 1500.0,
            very_far_dist: 3500.0,
            head: 1.25,
            leg: 0.8,
        },
        ballistics: Ballistics {
            speed: 19000.0,
            gravity_scale: 1.0,
            drag: 0.0,
            hit_radius: 0.0,
        },
        viewkick: Viewkick::default(),
    }
}

const DT: f32 = 1.0 / 165.0;

fn ground() -> WeaponInput {
    WeaponInput {
        on_ground: true,
        ..WeaponInput::default()
    }
}

fn firing() -> WeaponInput {
    WeaponInput {
        fire: true,
        ..ground()
    }
}

/// Run `secs` with `input`, collecting events.
fn run(s: &mut WeaponState, guns: &[GunSpec], input: WeaponInput, secs: f32) -> Vec<WeaponEvent> {
    let mut ev = Vec::new();
    for _ in 0..(secs / DT).round() as usize {
        ev.extend(s.step(guns, &input, DT));
    }
    ev
}

fn shots(ev: &[WeaponEvent]) -> usize {
    ev.iter()
        .filter(|e| matches!(e, WeaponEvent::Fired(_)))
        .count()
}

fn armed(guns: usize) -> WeaponState {
    WeaponState::with_slots([
        Some(Slot {
            gun: 0,
            mag: 17,
            reserve: 60,
            progress: 0,
        }),
        (guns > 1).then_some(Slot {
            gun: 1,
            mag: 5,
            reserve: 20,
            progress: 0,
        }),
    ])
}

#[test]
fn automatic_empties_the_mag_at_the_fire_rate_then_reloads() {
    let guns = [gun(FireMode::Auto, Charge::None)];
    let mut s = armed(1);
    // 17 rounds at 18/s: the 17th leaves at 16 intervals = 0.889 s.
    let ev = run(&mut s, &guns, firing(), 0.85);
    assert_eq!(shots(&ev), 16, "{s:?}");
    // The last round, then holding the trigger on the empty mag starts the
    // (empty) reload.
    let ev = run(&mut s, &guns, firing(), 0.2);
    assert_eq!(shots(&ev), 1);
    assert_eq!(s.slot().unwrap().mag, 0);
    assert!(
        ev.contains(&WeaponEvent::ReloadStarted { empty: true }),
        "{ev:?}"
    );
    let ev = run(&mut s, &guns, ground(), 2.5);
    assert!(ev.contains(&WeaponEvent::AmmoLoaded) && ev.contains(&WeaponEvent::ReloadFinished));
    assert_eq!((s.slot().unwrap().mag, s.slot().unwrap().reserve), (17, 43));
}

#[test]
fn semi_auto_fires_once_per_press() {
    let guns = [gun(FireMode::Semi, Charge::None)];
    let mut s = armed(1);
    assert_eq!(shots(&run(&mut s, &guns, firing(), 1.0)), 1);
    let mut n = 0;
    for _ in 0..5 {
        n += shots(&run(&mut s, &guns, ground(), 0.1));
        n += shots(&run(&mut s, &guns, firing(), 0.1));
    }
    assert_eq!(n, 5);
}

#[test]
fn burst_fires_its_count_then_waits() {
    let mut g = gun(
        FireMode::Burst {
            count: 3,
            delay: 0.28,
        },
        Charge::None,
    );
    g.fire_interval = 1.0 / 15.5;
    let guns = [g];
    let mut s = armed(1);
    assert_eq!(
        shots(&run(&mut s, &guns, firing(), 0.5)),
        3,
        "one press, one burst"
    );
    let _ = run(&mut s, &guns, ground(), 0.05);
    assert_eq!(shots(&run(&mut s, &guns, firing(), 0.6)), 3);
}

#[test]
fn pump_gun_waits_for_the_rechamber() {
    let mut g = gun(FireMode::Auto, Charge::None);
    g.fire_interval = 0.25;
    g.rechamber = 1.1;
    let guns = [g];
    let mut s = armed(1);
    assert_eq!(
        shots(&run(&mut s, &guns, firing(), 2.0)),
        2,
        "shots at 0 and 1.1 s"
    );
}

#[test]
fn switching_holsters_then_raises_the_other_slot() {
    let guns = [
        gun(FireMode::Auto, Charge::None),
        gun(FireMode::Semi, Charge::None),
    ];
    let mut s = armed(2);
    let select = WeaponInput {
        select: Some(1),
        ..ground()
    };
    let ev = run(&mut s, &guns, select, 0.2);
    assert_eq!(s.active, 0);
    assert!(ev.is_empty());
    let ev = run(&mut s, &guns, ground(), 0.5);
    assert!(ev.contains(&WeaponEvent::Holstered) && ev.contains(&WeaponEvent::Raised { slot: 1 }));
    assert_eq!(s.active, 1);
    // No shots while switching.
    let mut s2 = armed(2);
    let select_fire = WeaponInput {
        select: Some(1),
        fire: true,
        ..ground()
    };
    assert_eq!(shots(&run(&mut s2, &guns, select_fire, 0.5)), 0);
}

#[test]
fn charge_guns() {
    // Charge Rifle: fires by itself once charged.
    let guns = [gun(FireMode::Semi, Charge::FireWhenCharged { time: 0.85 })];
    let mut s = armed(1);
    assert_eq!(shots(&run(&mut s, &guns, firing(), 0.8)), 0);
    assert_eq!(shots(&run(&mut s, &guns, firing(), 0.1)), 1);
    // Bocek: fires on release, scaled by the draw.
    let guns = [gun(FireMode::Semi, Charge::ReleaseToFire { time: 0.45 })];
    let mut s = armed(1);
    let _ = run(&mut s, &guns, firing(), 0.2);
    let ev = run(&mut s, &guns, ground(), DT);
    let Some(WeaponEvent::Fired(shot)) = ev.first() else {
        panic!("{ev:?}")
    };
    assert!((shot.charge - 0.2 / 0.45).abs() < 0.03, "{}", shot.charge);
}

#[test]
fn spread_depends_on_stance_and_ads() {
    let g = gun(FireMode::Auto, Charge::None);
    let mut s = armed(1);
    let air = WeaponInput::default();
    assert_eq!(s.spread(&g, &ground()), 2.0);
    assert_eq!(s.spread(&g, &air), 7.0);
    s.ads = 1.0;
    assert!((s.spread(&g, &ground()) - 0.35).abs() < 1e-6);
    // Fixed blast patterns are used as-is (no randomness).
    let mut pk = gun(FireMode::Semi, Charge::None);
    pk.pellets = 3;
    pk.blast = vec![[0.0, 0.0], [-1.6, 0.0], [1.6, 0.0]];
    let guns = [pk];
    let mut s = armed(1);
    let ev = run(&mut s, &guns, firing(), DT);
    let Some(WeaponEvent::Fired(shot)) = ev.first() else {
        panic!()
    };
    assert_eq!(shot.pellets, vec![[0.0, 0.0], [-1.6, 0.0], [1.6, 0.0]]);
}

#[test]
fn bolts_drop_like_projectiles() {
    let g = gun(FireMode::Auto, Charge::None);
    let mut b = Bolt::launch([0.0; 3], [1.0, 0.0, 0.0], &g.ballistics, 0, 1, 1.0);
    let mut t = 0.0;
    while b.pos[0] < 3800.0 {
        b.step(&g.ballistics, DT);
        t += DT;
    }
    // ~100 m: 0.2 s of flight, drop g t^2 / 2 (no drag).
    let expect = 0.5 * apex_weapons::ballistics::GRAVITY * t * t;
    assert!(
        (-b.pos[2] - expect).abs() < 0.5,
        "{} vs {expect}",
        -b.pos[2]
    );
    assert_eq!(pellet_damage(&g, 500.0, HitZone::Head, 1.0), 11.0 * 1.25);
    assert_eq!(pellet_damage(&g, 500.0, HitZone::Legs, 1.0), 11.0 * 0.8);
}

#[test]
fn damage_falls_off_between_bands() {
    let mut g = gun(FireMode::Auto, Charge::None);
    g.damage = Damage {
        near: 75.0,
        far: 110.0,
        very_far: 110.0,
        near_dist: 1970.0,
        far_dist: 11820.0,
        very_far_dist: 11828.0,
        head: 2.0,
        leg: 0.9,
    };
    assert_eq!(g.damage.at(1000.0), 75.0);
    assert!((g.damage.at(6895.0) - 92.5).abs() < 0.1);
    assert_eq!(g.damage.at(20000.0), 110.0);
}

/// Every gun of the user's export, when present (never in the repo).
#[test]
fn personal_roster_loads_when_present() {
    let root = content();
    let root = root.as_path();
    let Ok(manifest) = std::fs::read_to_string(root.join("guns.json")) else {
        return;
    };
    let patterns =
        std::fs::read_to_string(root.join(r"weapons\common\weapon\viewkick_patterns.txt"))
            .map(|t| parse_patterns(&t))
            .unwrap_or_default();
    let blasts = std::fs::read_to_string(root.join(r"weapons\common\weapon\blast_patterns.txt"))
        .map(|t| parse_blast_patterns(&t))
        .unwrap_or_default();
    let mut n = 0;
    for line in manifest.lines().filter(|l| l.contains("\"definition\"")) {
        let rel = line.split('"').nth(3).unwrap();
        let id = rel.rsplit('/').next().unwrap().trim_end_matches(".txt");
        let def = WeaponDef::from_text(&std::fs::read_to_string(root.join(rel)).unwrap()).unwrap();
        let g = GunSpec::from_def(id, &def, &patterns, &blasts);
        eprintln!(
            "{id:28} {:?} rps {:5.2} clip {:3} pellets {:2} blast {:2} dmg {:5.1} speed {:6.0} pattern {}",
            g.fire_mode,
            1.0 / g.fire_interval,
            g.clip,
            g.pellets,
            g.blast.len(),
            g.damage.near,
            g.ballistics.speed,
            g.viewkick.pattern.as_ref().map_or(0, |p| p.bullets.len())
        );
        n += 1;
    }
    assert!(n >= 29);
}

// --- Reload, swap, sprint and inspect rules measured in Flowstate
// (research/weapons/behaviour_capture.py). ---

fn half_mag(guns: usize) -> WeaponState {
    let mut s = armed(guns);
    s.slots[0].as_mut().unwrap().mag = 8;
    s
}

fn press(mut i: WeaponInput) -> WeaponInput {
    i.reload = true;
    i
}

#[test]
fn reload_ammo_goes_in_then_the_gun_is_ready() {
    let guns = [gun(FireMode::Auto, Charge::None)];
    let mut s = half_mag(1);
    run(&mut s, &guns, press(ground()), DT);
    run(&mut s, &guns, ground(), 1.40);
    assert_eq!(s.slot().unwrap().mag, 8, "not in yet at 1.40 s");
    run(&mut s, &guns, ground(), 0.10);
    assert_eq!(s.slot().unwrap().mag, 17, "in by 1.50 s");
    assert!(matches!(s.activity, Activity::Reloading { .. }), "not ready before 1.61 s");
    run(&mut s, &guns, ground(), 0.15);
    assert_eq!(s.activity, Activity::Ready);
}

#[test]
fn swap_after_a_milestone_resumes_the_reload_when_drawn_again() {
    let guns = [gun(FireMode::Auto, Charge::None), gun(FireMode::Semi, Charge::None)];
    let mut s = half_mag(2);
    run(&mut s, &guns, press(ground()), DT);
    run(&mut s, &guns, ground(), 0.6); // past the 0.34 s milestone
    let swap = WeaponInput { select: Some(1), ..ground() };
    let ev = run(&mut s, &guns, swap, DT);
    assert!(ev.contains(&WeaponEvent::ReloadCancelled));
    assert_eq!(s.slots[0].unwrap().progress, 1);
    run(&mut s, &guns, ground(), 1.0);
    let back = WeaponInput { select: Some(0), ..ground() };
    run(&mut s, &guns, back, DT);
    let ev = run(&mut s, &guns, ground(), 0.7); // holster + raise, then resume
    assert!(ev.contains(&WeaponEvent::ReloadStarted { empty: false }), "{ev:?}");
    // The resumed reload is the short one: ammo in 0.8 s after it starts.
    let ev = run(&mut s, &guns, ground(), 0.9);
    assert!(ev.contains(&WeaponEvent::AmmoLoaded), "{ev:?}");
    assert_eq!(s.slot().unwrap().mag, 17);
}

#[test]
fn swap_before_a_milestone_starts_over() {
    let guns = [gun(FireMode::Auto, Charge::None), gun(FireMode::Semi, Charge::None)];
    let mut s = half_mag(2);
    run(&mut s, &guns, press(ground()), DT);
    run(&mut s, &guns, ground(), 0.2);
    run(&mut s, &guns, WeaponInput { select: Some(1), ..ground() }, DT);
    assert_eq!(s.slots[0].unwrap().progress, 0);
    assert_eq!(s.slots[0].unwrap().mag, 8);
}

#[test]
fn cancel_after_the_ammo_keeps_the_full_magazine() {
    let guns = [gun(FireMode::Auto, Charge::None), gun(FireMode::Semi, Charge::None)];
    let mut s = half_mag(2);
    run(&mut s, &guns, press(ground()), DT);
    run(&mut s, &guns, ground(), 1.5);
    run(&mut s, &guns, WeaponInput { select: Some(1), ..ground() }, DT);
    assert_eq!((s.slots[0].unwrap().mag, s.slots[0].unwrap().progress), (17, 0));
}

#[test]
fn melee_cancels_a_reload() {
    let guns = [gun(FireMode::Auto, Charge::None)];
    let mut s = half_mag(1);
    run(&mut s, &guns, press(ground()), DT);
    run(&mut s, &guns, ground(), 0.5);
    let ev = run(&mut s, &guns, WeaponInput { melee: true, ..ground() }, DT);
    assert!(ev.contains(&WeaponEvent::ReloadCancelled));
    assert_eq!(s.activity, Activity::Ready);
    assert_eq!(s.slot().unwrap().mag, 8);
}

#[test]
fn sprinting_and_aiming_do_not_cancel_a_reload_and_aim_waits_for_it() {
    let guns = [gun(FireMode::Auto, Charge::None)];
    let mut s = half_mag(1);
    run(&mut s, &guns, press(ground()), DT);
    run(&mut s, &guns, WeaponInput { sprinting: true, ..ground() }, 0.5);
    run(&mut s, &guns, WeaponInput { ads: true, ..ground() }, 0.9);
    assert!(matches!(s.activity, Activity::Reloading { .. }));
    assert_eq!(s.ads, 0.0, "no aiming during the reload");
    run(&mut s, &guns, WeaponInput { ads: true, ..ground() }, 0.4);
    assert_eq!(s.slot().unwrap().mag, 17);
    assert!(s.ads > 0.5, "aims once ready ({})", s.ads);
}

#[test]
fn hip_fire_out_of_a_sprint_waits_for_the_regrip() {
    let guns = [gun(FireMode::Auto, Charge::None)];
    let mut s = armed(1);
    run(&mut s, &guns, WeaponInput { sprinting: true, ..ground() }, 0.5);
    let ev = run(&mut s, &guns, firing(), 0.2);
    assert_eq!(shots(&ev), 0, "still regripping");
    let ev = run(&mut s, &guns, firing(), 0.1);
    assert!(shots(&ev) >= 1);
}

#[test]
fn firing_ends_an_inspect_at_once() {
    let guns = [gun(FireMode::Auto, Charge::None)];
    let mut s = armed(1);
    let ev = run(&mut s, &guns, WeaponInput { inspect: true, ..ground() }, DT);
    assert!(ev.contains(&WeaponEvent::InspectStarted));
    run(&mut s, &guns, ground(), 1.0);
    assert!(matches!(s.activity, Activity::Inspecting { .. }));
    let ev = run(&mut s, &guns, firing(), DT);
    assert_eq!(shots(&ev), 1, "fires on the same tick");
}

#[test]
fn guns_of_one_ammo_type_share_the_reserve() {
    let mut light = gun(FireMode::Auto, Charge::None);
    light.ammo = Some(0);
    let guns = [light.clone(), light];
    let mut s = armed(2);
    s.pools[0] = 20;
    s.slots[0].as_mut().unwrap().mag = 8;
    run(&mut s, &guns, press(ground()), DT);
    run(&mut s, &guns, ground(), 1.7);
    assert_eq!(s.slots[0].unwrap().mag, 17);
    assert_eq!(s.pools[0], 11, "the 9 rounds came from the shared pool");
    // The other gun reloads from what is left.
    run(&mut s, &guns, WeaponInput { select: Some(1), ..ground() }, DT);
    run(&mut s, &guns, ground(), 0.8);
    run(&mut s, &guns, press(ground()), DT);
    run(&mut s, &guns, ground(), 1.7);
    assert_eq!(s.slots[1].unwrap().mag, 16, "5 + 11 from the pool");
    assert_eq!(s.pools[0], 0);
    assert_eq!(s.slots[1].unwrap().reserve, 20, "the slot's own reserve is not used");
}

/// The personal content folder (`IW4L_APEX_CONTENT`, default `F:\ApexContent`).
fn content() -> std::path::PathBuf {
    std::env::var_os("IW4L_APEX_CONTENT").map_or_else(|| r"F:\ApexContent".into(), Into::into)
}
