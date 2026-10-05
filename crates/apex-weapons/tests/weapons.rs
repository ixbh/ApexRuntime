use apex_weapons::*;

const DEF: &str = r#"
"WeaponData"
{
	// val unk: 1
	"fire_rate" "18"
	"ammo_clip_size" "17"
	"damage_headshot_scale" "1.250000"
	"sway_min_pitch" "18446744073709551613"
	"viewkick_pattern" "test_2"
	"viewkick_pitch_base" "2.0"
	"viewkick_pitch_hardScale" "0.5"
	"viewkick_pitch_softScale" "4.0"
	"viewkick_yaw_base" "1.0"
	"viewkick_yaw_hardScale" "0.5"
	"viewkick_yaw_softScale" "2.0"
	"Mods"
	{
		"extended_mag"
		{
			"ammo_clip_size" "20"
		}
	}
}
"#;

const PATTERNS: &str = r#"
"ViewKickPatterns"
{
	"test_2"
	{
		"loop_offset" "1"
		"bullet_0" "0.10 -0.40 0.00 0.00"
		"bullet_1" "-0.20 -0.60 0.10 0.20"
		"bullet_2" "0.30 -0.50 0.00 0.00"
	}
}
"#;

#[test]
fn reads_weapon_numbers() {
    let d = WeaponDef::from_text(DEF).unwrap();
    assert_eq!(d.num("ammo_clip_size"), Some(17.0));
    assert_eq!(d.num("damage_headshot_scale"), Some(1.25));
    assert_eq!(d.num("sway_min_pitch"), Some(-3.0));
    assert!((d.fire_interval().unwrap() - 1.0 / 18.0).abs() < 1e-6);
    // Nested blocks do not leak into the top level.
    assert_eq!(
        d.data
            .block("Mods")
            .unwrap()
            .block("extended_mag")
            .unwrap()
            .num("ammo_clip_size"),
        Some(20.0)
    );
}

#[test]
fn pattern_loops_and_scales() {
    let pats = parse_patterns(PATTERNS);
    let p = &pats["test_2"];
    assert_eq!(p.bullets.len(), 3);
    assert_eq!(p.bullet(3), p.bullet(1));
    assert_eq!(p.bullet(4), p.bullet(2));
    let d = WeaponDef::from_text(DEF).unwrap();
    let vk = Viewkick::from_def(&d, &pats);
    assert!(vk.pattern.is_some());
    let k = vk.kick(0, || 0.0);
    // pitch -0.40 * base 2.0 = -0.8 -> hard 0.5x, soft 4x; yaw 0.1 * 1.0.
    assert!(
        (k.hard[0] + 0.4).abs() < 1e-6 && (k.soft[0] + 3.2).abs() < 1e-6,
        "{k:?}"
    );
    assert!((k.hard[1] - 0.05).abs() < 1e-6);
    // Randomness stays within the bullet's range.
    let mut rng = Rng::new(7);
    for _ in 0..100 {
        let k = vk.kick(1, || rng.signed());
        let pitch = k.hard[0] / 0.5 / 2.0;
        assert!((-0.8 - 1e-4..=-0.4 + 1e-4).contains(&pitch), "{pitch}");
    }
}

/// The user's personal exports, when present (never in the repo).
#[test]
fn personal_r99_when_present() {
    let w = std::path::Path::new(r"F:\ApexContent\weapons\r99\settings\weapon\mp_weapon_r97.txt");
    let p = std::path::Path::new(r"F:\ApexContent\weapons\common\weapon\viewkick_patterns.txt");
    if !(w.exists() && p.exists()) {
        return;
    }
    let d = WeaponDef::from_text(&std::fs::read_to_string(w).unwrap()).unwrap();
    let pats = parse_patterns(&std::fs::read_to_string(p).unwrap());
    let vk = Viewkick::from_def(&d, &pats);
    let pat = vk.pattern.as_ref().expect("r97 pattern");
    eprintln!(
        "R-99: {} rps, clip {}, pattern {} bullets",
        d.num_or("fire_rate", 0.0),
        d.num_or("ammo_clip_size", 0.0),
        pat.bullets.len()
    );
    let mut total = [0.0f32; 2];
    for shot in 0..d.num_or("ammo_clip_size", 0.0) as usize {
        let k = vk.kick(shot, || 0.0);
        total[0] += k.hard[0];
        total[1] += k.hard[1];
    }
    eprintln!(
        "full-mag aim drift (no randomness): pitch {:.2} deg, yaw {:.2} deg",
        total[0], total[1]
    );
}
