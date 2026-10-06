//! apex-weapons: an engine-free Apex weapon engine. Gun behaviour is read
//! at runtime from the user's personal RSX exports (`weapon/mp_weapon_*.txt`,
//! `viewkick_patterns.txt`, `blast_patterns.txt`); nothing here embeds Apex
//! data. [`spec`] turns a definition into a [`GunSpec`], [`sim`] runs a
//! player's weapons (slots, fire modes, reloads, spread, pellets, recoil)
//! deterministically, [`ballistics`] flies the bullets; hosts sweep the
//! bullet segments through their world and apply the damage.

use std::collections::HashMap;

pub mod ballistics;
pub mod sim;
pub mod spec;

pub use ballistics::{Bolt, HitZone, pellet_damage, pellet_dir};
pub use sim::{Activity, Shot, Slot, WeaponEvent, WeaponInput, WeaponState};
pub use spec::{
    Ballistics, Charge, Damage, FireMode, GunSpec, ReloadPlan, Reloads, Spread, parse_blast_patterns,
};

/// A KeyValues block: `"key" "value"` pairs and nested `"name" { ... }`
/// blocks (as RSX writes `WeaponData` and the viewkick pattern table).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KvBlock {
    pub values: Vec<(String, String)>,
    pub blocks: Vec<(String, KvBlock)>,
}

impl KvBlock {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    pub fn block(&self, name: &str) -> Option<&KvBlock> {
        self.blocks
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, b)| b)
    }

    /// Number value. Negative integers exported as unsigned 64-bit
    /// (`18446744073709551615` = -1) are wrapped back.
    pub fn num(&self, key: &str) -> Option<f32> {
        let v = self.get(key)?.trim();
        if let Ok(f) = v.parse::<f64>() {
            if f > i64::MAX as f64
                && let Ok(u) = v.parse::<u64>()
            {
                return Some(u as i64 as f32);
            }
            return Some(f as f32);
        }
        None
    }

    pub fn num_or(&self, key: &str, default: f32) -> f32 {
        self.num(key).unwrap_or(default)
    }

    /// Whitespace-separated numbers (`"0.21 -0.37 0.05 0.05"`).
    pub fn nums(&self, key: &str) -> Vec<f32> {
        self.get(key)
            .map(|v| {
                v.split_whitespace()
                    .filter_map(|x| x.parse().ok())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Parse KeyValues text. Unknown syntax is skipped rather than failing.
pub fn parse_kv(text: &str) -> KvBlock {
    let mut tokens = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some(&(i, c)) = chars.peek() {
        match c {
            '/' if text[i..].starts_with("//") => {
                while let Some(&(_, c)) = chars.peek() {
                    if c == '\n' {
                        break;
                    }
                    chars.next();
                }
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                for (_, c) in chars.by_ref() {
                    if c == '"' {
                        break;
                    }
                    s.push(c);
                }
                tokens.push(Some(s));
            }
            '{' => {
                chars.next();
                tokens.push(Some("\u{1}{".into()));
            }
            '}' => {
                chars.next();
                tokens.push(Some("\u{1}}".into()));
            }
            _ => {
                chars.next();
            }
        }
    }
    let tokens: Vec<String> = tokens.into_iter().flatten().collect();
    let mut at = 0;
    fn block(tokens: &[String], at: &mut usize) -> KvBlock {
        let mut b = KvBlock::default();
        while *at < tokens.len() {
            let t = &tokens[*at];
            if t == "\u{1}}" {
                *at += 1;
                return b;
            }
            if t == "\u{1}{" {
                // Anonymous block: parse and drop.
                *at += 1;
                let _ = block(tokens, at);
                continue;
            }
            let key = t.clone();
            *at += 1;
            match tokens.get(*at) {
                Some(n) if n == "\u{1}{" => {
                    *at += 1;
                    let child = block(tokens, at);
                    b.blocks.push((key, child));
                }
                Some(n) if n != "\u{1}}" => {
                    b.values.push((key, n.clone()));
                    *at += 1;
                }
                _ => {}
            }
        }
        b
    }
    block(&tokens, &mut at)
}

/// The weapon definition: the `WeaponData` block of an exported
/// `mp_weapon_*.txt`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponDef {
    pub data: KvBlock,
}

impl WeaponDef {
    pub fn from_text(text: &str) -> Option<WeaponDef> {
        let root = parse_kv(text);
        let data = root.block("WeaponData")?.clone();
        Some(WeaponDef { data })
    }

    pub fn num(&self, key: &str) -> Option<f32> {
        self.data.num(key)
    }

    pub fn num_or(&self, key: &str, default: f32) -> f32 {
        self.data.num_or(key, default)
    }

    pub fn str(&self, key: &str) -> Option<&str> {
        self.data.get(key).filter(|s| !s.is_empty())
    }

    /// Seconds between shots (`fire_rate` is shots per second).
    pub fn fire_interval(&self) -> Option<f32> {
        self.num("fire_rate").filter(|r| *r > 0.0).map(|r| 1.0 / r)
    }
}

/// One recoil pattern: per-bullet `[yaw, pitch, yaw_random, pitch_random]`
/// in degrees (pitch negative = up), looping back to `loop_offset` after
/// the last bullet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecoilPattern {
    pub bullets: Vec<[f32; 4]>,
    pub loop_offset: usize,
}

impl RecoilPattern {
    pub fn bullet(&self, shot: usize) -> Option<[f32; 4]> {
        let n = self.bullets.len();
        if n == 0 {
            return None;
        }
        let i = if shot < n {
            shot
        } else {
            let lo = self.loop_offset.min(n - 1);
            lo + (shot - lo) % (n - lo)
        };
        self.bullets.get(i).copied()
    }
}

/// All patterns from `viewkick_patterns.txt` (top-level blocks by name).
pub fn parse_patterns(text: &str) -> HashMap<String, RecoilPattern> {
    let root = parse_kv(text);
    let mut out = HashMap::new();
    let mut collect = |name: &str, b: &KvBlock| {
        let mut bullets = Vec::new();
        for i in 0.. {
            let v = b.nums(&format!("bullet_{i}"));
            if v.is_empty() {
                break;
            }
            let mut a = [0.0; 4];
            for (d, s) in a.iter_mut().zip(v) {
                *d = s;
            }
            bullets.push(a);
        }
        if !bullets.is_empty() {
            out.insert(
                name.to_ascii_lowercase(),
                RecoilPattern {
                    bullets,
                    loop_offset: b.num_or("loop_offset", 0.0).max(0.0) as usize,
                },
            );
        }
    };
    for (name, b) in &root.blocks {
        collect(name, b);
        for (inner, ib) in &b.blocks {
            collect(inner, ib);
        }
    }
    out
}

/// Recoil for one shot, split the way Respawn's viewkick does: `hard` moves
/// the aim (the player pulls it back down), `soft` is a visual punch that
/// springs back. Degrees, `[pitch, yaw]`, pitch negative = up.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Kick {
    pub hard: [f32; 2],
    pub soft: [f32; 2],
}

/// Viewkick parameters from a weapon definition.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Viewkick {
    pub pattern: Option<RecoilPattern>,
    pub pitch_base: f32,
    pub pitch_random: f32,
    pub pitch_soft: f32,
    pub pitch_hard: f32,
    pub yaw_base: f32,
    pub yaw_random: f32,
    pub yaw_soft: f32,
    pub yaw_hard: f32,
    pub ads_air_scale: f32,
}

impl Viewkick {
    pub fn from_def(def: &WeaponDef, patterns: &HashMap<String, RecoilPattern>) -> Viewkick {
        Viewkick {
            pattern: def
                .str("viewkick_pattern")
                .and_then(|p| patterns.get(&p.to_ascii_lowercase()).cloned()),
            pitch_base: def.num_or("viewkick_pitch_base", 0.0),
            pitch_random: def.num_or("viewkick_pitch_random", 0.0),
            pitch_soft: def.num_or("viewkick_pitch_softScale", 1.0),
            pitch_hard: def.num_or("viewkick_pitch_hardScale", 0.0),
            yaw_base: def.num_or("viewkick_yaw_base", 0.0),
            yaw_random: def.num_or("viewkick_yaw_random", 0.0),
            yaw_soft: def.num_or("viewkick_yaw_softScale", 1.0),
            yaw_hard: def.num_or("viewkick_yaw_hardScale", 0.0),
            ads_air_scale: def.num_or("viewkick_air_scale_ads", 1.0),
        }
    }

    /// Kick for the `shot`-th bullet of a spray (0 = first). `rand` returns
    /// uniform numbers in [-1, 1]. With a pattern, the pattern gives the
    /// direction and its own random range, scaled by the base values;
    /// without one, base ± random.
    pub fn kick(&self, shot: usize, mut rand: impl FnMut() -> f32) -> Kick {
        let (pitch, yaw) = match self.pattern.as_ref().and_then(|p| p.bullet(shot)) {
            Some([yaw, pitch, yaw_r, pitch_r]) => (
                (pitch + pitch_r * rand()) * self.pitch_base,
                (yaw + yaw_r * rand()) * self.yaw_base,
            ),
            None => (
                -(self.pitch_base + self.pitch_random * rand().abs()),
                self.yaw_random * rand(),
            ),
        };
        Kick {
            hard: [pitch * self.pitch_hard, yaw * self.yaw_hard],
            soft: [pitch * self.pitch_soft, yaw * self.yaw_soft],
        }
    }
}

/// Small deterministic RNG for recoil randomness (xorshift).
#[derive(Clone, Copy, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed | 1)
    }

    /// Uniform in [-1, 1].
    pub fn signed(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0
    }
}
