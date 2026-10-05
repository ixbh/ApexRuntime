//! Per-player weapon simulation: two slots, raise/holster, reload, fire
//! modes, rechamber, charge, ADS, spread, pellets and recoil. Deterministic:
//! the same state + inputs + dt give the same result (hosts run it for
//! server authority and client prediction alike).

use crate::spec::{Charge, FireMode, GunSpec};
use crate::{Kick, Rng};

/// What the player is doing this tick, as the weapon cares about it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WeaponInput {
    pub fire: bool,
    pub ads: bool,
    pub reload: bool,
    /// Switch to this slot (0 or 1).
    pub select: Option<usize>,
    pub on_ground: bool,
    pub crouched: bool,
    pub sprinting: bool,
    /// Horizontal speed, units per second.
    pub speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    /// Index into the host's gun table.
    pub gun: usize,
    pub mag: u32,
    pub reserve: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Activity {
    #[default]
    Ready,
    Raising {
        left: f32,
    },
    Holstering {
        left: f32,
        to: usize,
    },
    Reloading {
        t: f32,
        empty: bool,
        loaded: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponState {
    pub slots: [Option<Slot>; 2],
    pub active: usize,
    pub activity: Activity,
    /// Seconds until the next shot may fire.
    pub cooldown: f32,
    /// Shots left in the current burst.
    pub burst_left: u32,
    /// Charge drawn so far (seconds), while charging.
    pub charge: f32,
    pub trigger_was_down: bool,
    /// Shots in the current spray (recoil pattern index) and time since one.
    pub shot_index: u32,
    pub since_shot: f32,
    pub spread_kick: f32,
    /// 0 = hip, 1 = fully aimed down sights.
    pub ads: f32,
    pub rng: u64,
}

impl Default for WeaponState {
    fn default() -> Self {
        Self {
            slots: [None, None],
            active: 0,
            activity: Activity::Ready,
            cooldown: 0.0,
            burst_left: 0,
            charge: 0.0,
            trigger_was_down: false,
            shot_index: 0,
            since_shot: f32::MAX,
            spread_kick: 0.0,
            ads: 0.0,
            rng: 0x9E37_79B9_7F4A_7C15,
        }
    }
}

/// A fired shot: pellet directions as offsets from the aim, degrees
/// `[right, up]`, and the recoil it causes.
#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    pub gun: usize,
    pub shot_index: u32,
    pub pellets: Vec<[f32; 2]>,
    pub kick: Kick,
    /// Charge fraction for charged guns (1 otherwise); scales damage/speed.
    pub charge: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WeaponEvent {
    Fired(Shot),
    DryFire,
    ReloadStarted { empty: bool },
    AmmoLoaded,
    ReloadFinished,
    Holstered,
    Raised { slot: usize },
}

/// Recoil spray resets after this long without a shot (seconds).
pub const SPRAY_RESET: f32 = 0.25;

impl WeaponState {
    pub fn with_slots(slots: [Option<Slot>; 2]) -> Self {
        let mut s = Self {
            slots,
            ..Self::default()
        };
        s.active = if s.slots[0].is_some() { 0 } else { 1 };
        s
    }

    pub fn slot(&self) -> Option<&Slot> {
        self.slots[self.active].as_ref()
    }

    fn rand(&mut self) -> f32 {
        let mut r = Rng::new(self.rng);
        let v = r.signed();
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        v
    }

    /// Current spread cone half-angle, degrees.
    pub fn spread(&self, spec: &GunSpec, input: &WeaponInput) -> f32 {
        let s = &spec.spread;
        let hip = if !input.on_ground {
            s.air_hip
        } else if input.sprinting {
            s.sprint_hip
        } else if input.crouched {
            s.crouch_hip
        } else if input.speed > 50.0 {
            s.run_hip
        } else {
            s.stand_hip
        };
        let ads = if !input.on_ground {
            s.air_ads
        } else if input.crouched {
            s.crouch_ads
        } else {
            s.stand_ads
        };
        hip + (ads - hip) * self.ads + self.spread_kick * (1.0 - self.ads)
    }

    /// Advance by `dt` seconds with `guns` (the host's gun table).
    pub fn step(&mut self, guns: &[GunSpec], input: &WeaponInput, dt: f32) -> Vec<WeaponEvent> {
        let events = self.step_inner(guns, input, dt);
        if !events.iter().any(|e| matches!(e, WeaponEvent::Fired(_))) {
            self.cooldown = self.cooldown.max(0.0);
        }
        events
    }

    fn step_inner(&mut self, guns: &[GunSpec], input: &WeaponInput, dt: f32) -> Vec<WeaponEvent> {
        let mut events = Vec::new();
        // The cooldown may dip below zero within the tick a shot fires, so
        // the fraction carries into the next interval (exact fire rates at
        // any frame rate); it is clamped back to zero when no shot fires.
        self.cooldown -= dt;
        self.since_shot += dt;
        if self.since_shot > SPRAY_RESET {
            self.shot_index = 0;
        }
        let Some(spec) = self.slot().and_then(|s| guns.get(s.gun)) else {
            return events;
        };

        // Spread kick decays after a pause.
        if self.since_shot > spec.spread.decay_delay {
            self.spread_kick = (self.spread_kick - spec.spread.decay_rate * dt).max(0.0);
        }

        // Switching: holster, then raise the other slot.
        if let Some(to) = input.select
            && to != self.active
            && to < 2
            && self.slots[to].is_some()
            && !matches!(self.activity, Activity::Holstering { .. })
        {
            self.activity = Activity::Holstering {
                left: spec.holster,
                to,
            };
            self.charge = 0.0;
            self.burst_left = 0;
        }

        // ADS moves toward the input (no ADS while switching or sprinting).
        let want_ads = input.ads
            && !input.sprinting
            && !matches!(
                self.activity,
                Activity::Holstering { .. } | Activity::Raising { .. }
            );
        let rate = if want_ads {
            1.0 / spec.zoom_in
        } else {
            -1.0 / spec.zoom_out
        };
        self.ads = (self.ads + rate * dt).clamp(0.0, 1.0);

        match self.activity {
            Activity::Holstering { left, to } => {
                let left = left - dt;
                if left <= 0.0 {
                    self.active = to;
                    events.push(WeaponEvent::Holstered);
                    let raise = self
                        .slot()
                        .and_then(|s| guns.get(s.gun))
                        .map_or(0.3, |g| g.raise);
                    self.activity = Activity::Raising { left: raise };
                    self.cooldown = 0.0;
                    self.shot_index = 0;
                    self.spread_kick = 0.0;
                } else {
                    self.activity = Activity::Holstering { left, to };
                }
                self.trigger_was_down = input.fire;
                return events;
            }
            Activity::Raising { left } => {
                let left = left - dt;
                if left <= 0.0 {
                    self.activity = Activity::Ready;
                    events.push(WeaponEvent::Raised { slot: self.active });
                } else {
                    self.activity = Activity::Raising { left };
                    self.trigger_was_down = input.fire;
                    return events;
                }
            }
            Activity::Reloading { t, empty, loaded } => {
                let t = t + dt;
                let (total, add) = if empty {
                    (spec.reload_empty, spec.reload_empty_add)
                } else {
                    (spec.reload, spec.reload_add)
                };
                let mut loaded = loaded;
                if !loaded && t >= add {
                    loaded = true;
                    let slot = self.slots[self.active].as_mut().expect("active slot");
                    let need = spec.clip.saturating_sub(slot.mag);
                    let take = need.min(slot.reserve);
                    slot.mag += take;
                    slot.reserve -= take;
                    events.push(WeaponEvent::AmmoLoaded);
                }
                if t >= total {
                    self.activity = Activity::Ready;
                    events.push(WeaponEvent::ReloadFinished);
                } else {
                    self.activity = Activity::Reloading { t, empty, loaded };
                    self.trigger_was_down = input.fire;
                    return events;
                }
            }
            Activity::Ready => {}
        }

        // Ready: reload on request, or when the trigger meets an empty mag.
        let slot = *self.slot().expect("active slot");
        let can_reload = slot.mag < spec.clip && slot.reserve > 0;
        let empty = slot.mag < spec.ammo_per_shot;
        if can_reload && (input.reload || (empty && input.fire && self.cooldown <= 0.0)) {
            self.activity = Activity::Reloading {
                t: 0.0,
                empty: slot.mag == 0,
                loaded: false,
            };
            self.charge = 0.0;
            self.burst_left = 0;
            events.push(WeaponEvent::ReloadStarted {
                empty: slot.mag == 0,
            });
            self.trigger_was_down = input.fire;
            return events;
        }

        let pressed = input.fire && !self.trigger_was_down;
        let released = !input.fire && self.trigger_was_down;
        self.trigger_was_down = input.fire;

        // Decide whether a shot goes off this tick, and its charge.
        let mut fire_charge = None;
        match spec.charge {
            Charge::ReleaseToFire { time } => {
                if input.fire && self.cooldown <= 0.0 {
                    self.charge = (self.charge + dt).min(time);
                }
                if released && self.charge > 0.0 {
                    fire_charge = Some((self.charge / time).clamp(0.0, 1.0));
                    self.charge = 0.0;
                }
            }
            Charge::FireWhenCharged { time } => {
                if input.fire && self.cooldown <= 0.0 {
                    self.charge += dt;
                    if self.charge >= time {
                        fire_charge = Some(1.0);
                        self.charge = 0.0;
                    }
                } else {
                    self.charge = 0.0;
                }
            }
            Charge::None => {
                if self.cooldown <= 0.0 {
                    let go = match spec.fire_mode {
                        FireMode::Auto => input.fire,
                        FireMode::Semi => pressed,
                        FireMode::Burst { count, .. } => {
                            if self.burst_left == 0 && pressed {
                                self.burst_left = count;
                            }
                            self.burst_left > 0
                        }
                    };
                    if go {
                        fire_charge = Some(1.0);
                    }
                }
            }
        }

        if let Some(charge) = fire_charge {
            if empty {
                events.push(WeaponEvent::DryFire);
                self.burst_left = 0;
                return events;
            }
            let spread = self.spread(spec, input);
            let pellets = if spec.blast.is_empty() {
                (0..spec.pellets)
                    .map(|_| {
                        // Uniform in the cone's disk.
                        let a = self.rand() * std::f32::consts::PI;
                        let r = spread * self.rand().abs().sqrt();
                        [r * a.cos(), r * a.sin()]
                    })
                    .collect()
            } else {
                spec.blast
                    .iter()
                    .take(spec.pellets.max(1) as usize)
                    .copied()
                    .collect()
            };
            let kick = spec.viewkick.kick(self.shot_index as usize, || 0.0);
            let slot = self.slots[self.active].as_mut().expect("active slot");
            slot.mag -= spec.ammo_per_shot;
            events.push(WeaponEvent::Fired(Shot {
                gun: slot.gun,
                shot_index: self.shot_index,
                pellets,
                kick,
                charge,
            }));
            self.shot_index += 1;
            self.since_shot = 0.0;
            self.spread_kick = (self.spread_kick + spec.spread.kick_hip).min(spec.spread.max_kick);
            let mut cooldown = spec.fire_interval.max(spec.rechamber);
            if let FireMode::Burst { delay, .. } = spec.fire_mode {
                self.burst_left = self.burst_left.saturating_sub(1);
                if self.burst_left == 0 {
                    cooldown += delay;
                }
            }
            // Keep the sub-tick remainder (cooldown is <= 0 here).
            self.cooldown += cooldown;
        }
        events
    }
}
