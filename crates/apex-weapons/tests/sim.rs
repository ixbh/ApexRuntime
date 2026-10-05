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
        raise: 0.3,
        holster: 0.3,
        zoom_in: 0.2,
        zoom_out: 0.2,
        zoom_fov: 60.0,
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
        }),
        (guns > 1).then_some(Slot {
            gun: 1,
            mag: 5,
            reserve: 20,
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
    let root = std::path::Path::new(r"F:\ApexContent");
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
