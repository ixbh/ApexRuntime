//! A gun's behaviour, read once from its exported definition.

use std::collections::HashMap;

use crate::{KvBlock, RecoilPattern, Viewkick, WeaponDef, parse_kv};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FireMode {
    /// Fires while the trigger is held.
    Auto,
    /// One shot per trigger press.
    Semi,
    /// `count` shots per press, `fire_interval` apart, then `delay`.
    Burst { count: u32, delay: f32 },
}

/// How a charged gun releases its shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Charge {
    None,
    /// Fires on trigger release, scaled by how far it was drawn (Bocek).
    ReleaseToFire {
        time: f32,
    },
    /// Fires by itself once fully charged while held (Charge Rifle).
    FireWhenCharged {
        time: f32,
    },
}

/// Hip and ADS spread, degrees (cone half-angle), by stance.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spread {
    pub stand_hip: f32,
    pub run_hip: f32,
    pub sprint_hip: f32,
    pub crouch_hip: f32,
    pub air_hip: f32,
    pub stand_ads: f32,
    pub crouch_ads: f32,
    pub air_ads: f32,
    /// Added per shot (hip), capped at `max_kick`.
    pub kick_hip: f32,
    pub max_kick: f32,
    pub decay_delay: f32,
    /// Degrees per second.
    pub decay_rate: f32,
}

/// Damage per pellet: `near` up to `near_dist`, linear to `far` at
/// `far_dist`, linear to `very_far` at `very_far_dist`, flat after.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Damage {
    pub near: f32,
    pub far: f32,
    pub very_far: f32,
    pub near_dist: f32,
    pub far_dist: f32,
    pub very_far_dist: f32,
    pub head: f32,
    pub leg: f32,
}

impl Damage {
    pub fn at(&self, dist: f32) -> f32 {
        let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t.clamp(0.0, 1.0);
        if dist <= self.near_dist {
            self.near
        } else if dist <= self.far_dist {
            lerp(
                self.near,
                self.far,
                (dist - self.near_dist) / (self.far_dist - self.near_dist).max(1e-3),
            )
        } else if dist <= self.very_far_dist {
            lerp(
                self.far,
                self.very_far,
                (dist - self.far_dist) / (self.very_far_dist - self.far_dist).max(1e-3),
            )
        } else {
            self.very_far
        }
    }
}

/// Bullet flight: Apex bullets are projectiles ("bolts").
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ballistics {
    /// Units per second.
    pub speed: f32,
    /// Multiplies world gravity.
    pub gravity_scale: f32,
    pub drag: f32,
    /// Hit radius (units); 0 = a ray.
    pub hit_radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GunSpec {
    /// Internal name (`mp_weapon_r97`).
    pub id: String,
    pub fire_mode: FireMode,
    pub charge: Charge,
    /// Seconds between shots (`1 / fire_rate`).
    pub fire_interval: f32,
    /// Pump/bolt time after each shot; the next shot waits for it.
    pub rechamber: f32,
    pub clip: u32,
    pub ammo_per_shot: u32,
    pub pellets: u32,
    /// Fixed pellet offsets, degrees `[right, up]` (empty = random in the
    /// spread cone).
    pub blast: Vec<[f32; 2]>,
    /// Reload: total time and when the ammo goes in (seconds).
    pub reload: f32,
    pub reload_add: f32,
    pub reload_empty: f32,
    pub reload_empty_add: f32,
    /// Reload gameplay points per animation (measured in Flowstate where
    /// available, see [`Reloads`]).
    pub reloads: Reloads,
    pub raise: f32,
    pub holster: f32,
    /// Hip fire out of a sprint waits this long (the regrip).
    pub sprint_out: f32,
    /// Weapon inspect length (any action cancels it).
    pub inspect: f32,
    pub zoom_in: f32,
    pub zoom_out: f32,
    pub zoom_fov: f32,
    pub ads_move_scale: f32,
    pub spread: Spread,
    pub damage: Damage,
    pub ballistics: Ballistics,
    pub viewkick: Viewkick,
}

/// One reload animation's gameplay points, seconds from its start.
#[derive(Clone, Debug, PartialEq)]
pub struct ReloadPlan {
    /// The magazine fills; cancelling after this keeps the ammo.
    pub ammo_in: f32,
    /// The gun can fire and aim again (the animation runs on a little).
    pub ready: f32,
    /// Progress points: a reload cancelled after milestone `k` (1-based)
    /// resumes later with `resume[k - 1]`.
    pub milestones: Vec<f32>,
}

impl ReloadPlan {
    /// Milestones passed at time `t`.
    pub fn passed(&self, t: f32) -> u8 {
        self.milestones.iter().filter(|&&m| t >= m).count() as u8
    }
}

/// A gun's reloads (Flowstate behaviour, research/weapons/behaviour_*):
/// swapping or meleeing cancels a reload; one cancelled after a milestone
/// but before the ammo resumes from that milestone (a shorter animation)
/// as soon as the gun is drawn again; sprinting, sliding, jumping, aiming
/// and firing never cancel it, and aiming waits until it is ready.
#[derive(Clone, Debug, PartialEq)]
pub struct Reloads {
    pub tactical: ReloadPlan,
    pub empty: ReloadPlan,
    /// Resumes after tactical milestone 1, 2, ...
    pub resume: Vec<ReloadPlan>,
    pub empty_resume: Vec<ReloadPlan>,
}

impl Reloads {
    /// Estimates from the weapon file when nothing was measured: ammo at
    /// 70%, ready at 90%, one milestone at 20%; the resume is the file's
    /// `reload_time_late1` long.
    pub fn estimate(reload: f32, empty: f32, late1: f32, empty_late1: f32) -> Reloads {
        let plan = |t: f32, milestones: Vec<f32>| ReloadPlan {
            ammo_in: 0.7 * t,
            ready: 0.9 * t,
            milestones,
        };
        Reloads {
            tactical: plan(reload, vec![0.2 * reload]),
            empty: plan(empty, vec![0.2 * empty]),
            resume: vec![plan(late1, Vec::new())],
            empty_resume: vec![plan(empty_late1, Vec::new())],
        }
    }

    /// The plan for a reload of an empty or partial magazine, fresh
    /// (`resume_from` 0) or resuming after that milestone.
    pub fn plan(&self, empty: bool, resume_from: u8) -> &ReloadPlan {
        let (full, resumes) = if empty {
            (&self.empty, &self.empty_resume)
        } else {
            (&self.tactical, &self.resume)
        };
        match resume_from {
            0 => full,
            k => resumes.get(k as usize - 1).or(resumes.last()).unwrap_or(full),
        }
    }
}

/// Blast patterns from `blast_patterns.txt`: name -> raw `[x, y]` points.
pub fn parse_blast_patterns(text: &str) -> HashMap<String, Vec<[f32; 2]>> {
    let root = parse_kv(text);
    let mut out = HashMap::new();
    let mut collect = |name: &str, b: &KvBlock| {
        let pts: Vec<[f32; 2]> = (0..)
            .map(|i| b.nums(&format!("bullet_{i}")))
            .take_while(|v| !v.is_empty())
            .map(|v| [v[0], v.get(1).copied().unwrap_or(0.0)])
            .collect();
        if !pts.is_empty() {
            out.insert(name.to_ascii_lowercase(), pts);
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

/// Blast pattern units: tenths of a degree, times the gun's
/// `blast_pattern_default_scale` (estimate; the wall test confirms).
pub const BLAST_UNIT_DEG: f32 = 0.1;

impl GunSpec {
    pub fn from_def(
        id: &str,
        def: &WeaponDef,
        patterns: &HashMap<String, RecoilPattern>,
        blasts: &HashMap<String, Vec<[f32; 2]>>,
    ) -> GunSpec {
        let n = |k: &str, d: f32| def.num_or(k, d);
        let fire_rate = n("fire_rate", 10.0).max(0.01);
        let semi = n("is_semi_auto", 0.0) != 0.0;
        let burst = n("burst_fire_count", 0.0) as u32;
        let fire_mode = if burst > 1 {
            FireMode::Burst {
                count: burst,
                delay: n("burst_fire_delay", 0.0),
            }
        } else if semi {
            FireMode::Semi
        } else {
            FireMode::Auto
        };
        let charge_time = n("charge_time", 0.0);
        let charge = if charge_time <= 0.0 {
            Charge::None
        } else if id.contains("bow") {
            Charge::ReleaseToFire { time: charge_time }
        } else {
            Charge::FireWhenCharged { time: charge_time }
        };
        let scale = n("blast_pattern_default_scale", 1.0);
        let blast = def
            .str("blast_pattern")
            .and_then(|b| blasts.get(&b.to_ascii_lowercase()))
            .map(|pts| {
                pts.iter()
                    .map(|p| [p[0] * scale * BLAST_UNIT_DEG, p[1] * scale * BLAST_UNIT_DEG])
                    .collect()
            })
            .unwrap_or_default();
        let reload = n("reload_time", 2.0);
        let reload_empty = n("reloadempty_time", reload);
        let near = n("damage_near_value", 10.0);
        let far = n("damage_far_value", near);
        let stand_hip = n("spread_stand_hip", 2.0);
        GunSpec {
            id: id.to_owned(),
            fire_mode,
            charge,
            fire_interval: 1.0 / fire_rate,
            rechamber: n("rechamber_time", 0.0),
            clip: n("ammo_clip_size", 20.0).max(1.0) as u32,
            ammo_per_shot: n("ammo_per_shot", 1.0).max(1.0) as u32,
            pellets: n("projectiles_per_shot", 1.0).max(1.0) as u32,
            blast,
            reload,
            reload_add: reload - n("reload_time_late1", reload * 0.45),
            reload_empty,
            reload_empty_add: reload_empty - n("reloadempty_time_late1", reload_empty * 0.6),
            reloads: Reloads::estimate(
                reload,
                reload_empty,
                n("reload_time_late1", reload * 0.45),
                n("reloadempty_time_late1", reload_empty * 0.6),
            ),
            raise: n("raise_time", 0.3),
            holster: n("holster_time", 0.3),
            sprint_out: 0.24,
            inspect: 4.0,
            zoom_in: n("zoom_time_in", 0.2).max(0.01),
            zoom_out: n("zoom_time_out", 0.2).max(0.01),
            zoom_fov: n("zoom_fov", 60.0),
            ads_move_scale: n("ads_move_speed_scale", 1.0),
            spread: Spread {
                stand_hip,
                run_hip: n("spread_stand_hip_run", stand_hip),
                sprint_hip: n("spread_stand_hip_sprint", stand_hip),
                crouch_hip: n("spread_crouch_hip", stand_hip),
                air_hip: n("spread_air_hip", stand_hip),
                stand_ads: n("spread_stand_ads", 0.0),
                crouch_ads: n("spread_crouch_ads", n("spread_stand_ads", 0.0)),
                air_ads: n("spread_air_ads", n("spread_stand_ads", 0.0)),
                kick_hip: n("spread_kick_on_fire_stand_hip", 0.0),
                max_kick: n("spread_max_kick_stand_hip", 0.0),
                decay_delay: n("spread_decay_delay", 0.15),
                decay_rate: n("spread_decay_rate", 10.0),
            },
            damage: Damage {
                near,
                far,
                very_far: n("damage_very_far_value", far),
                near_dist: n("damage_near_distance", 1000.0),
                far_dist: n("damage_far_distance", 2500.0),
                very_far_dist: n("damage_very_far_distance", n("damage_far_distance", 2500.0)),
                head: n("damage_headshot_scale", 1.0),
                leg: n("damage_leg_scale", 1.0),
            },
            ballistics: Ballistics {
                speed: n("projectile_launch_speed", 20000.0).max(1.0),
                gravity_scale: n("projectile_gravity_scale", 1.0),
                drag: n("projectile_drag_coefficient", 0.0),
                hit_radius: n("bolt_hitsize", 0.0),
            },
            viewkick: Viewkick::from_def(def, patterns),
        }
    }
}
