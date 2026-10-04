//! apex-sandbox — first-person test bed for apex-core.
//!
//! Bevy is a *host* here, exactly like a future IW4L or Unity adapter:
//! it samples input, drives `apex_core::step` on a fixed clock, and renders
//! the resulting state. Nothing in the movement depends on Bevy.
//!
//! Controls: mouse look · WASD · Space jump · Ctrl/C/L crouch · Shift sprint
//!           scroll up = tap W · scroll down = tap jump (Apex-style binds)
//!           F1 HUD · F2 always-sprint · F3 first/third person · F4 camera feel
//!           F5 start/stop recording · F6 movement trail · F7 jump fatigue
//!           R respawn
//!           Esc release mouse
//! Options:  --fps N   cap the frame rate (like Apex's fps_max; default 165)
//!           --vsync   no cap, sync to the monitor instead
//!           --hz N    fixed simulation rate instead of one tick per frame
//!           --fov N   Apex FOV slider value (default 110)
//!           --sens N  Apex mouse sensitivity (default 2.0; 0.022° per count × N,
//!                     same mouse/DPI as in game)
//!
//! Like Apex, movement runs once per rendered frame (one usercmd per frame,
//! dt = that frame's time), so frame-rate-dependent tech behaves as in game.
//! Input is sampled the way Source builds a usercmd: a scroll notch (press +
//! release inside one frame) holds the button for that frame's tick only;
//! notches in back-to-back ticks read as a held button, exactly as in game.

mod brush_mesh;
mod view;

use std::path::PathBuf;

use apex_collision::BrushWorld;
use apex_collision::maps::{SPAWN, proving_ground};
use apex_core::{
    Buttons, FixedClock, MovementConfig, MovementInput, MovementState, StepReport, step_with_report,
};
use apex_telemetry::{InputEvent, InputScript, ScriptStart, TraceRow, write_csv};
use bevy::input::mouse::{AccumulatedMouseMotion, MouseWheel};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

/// Frame-locked dt limits: a hitch longer than this is simulated as this.
const MIN_FRAME_DT: f64 = 1.0 / 1000.0;
const MAX_FRAME_DT: f64 = 1.0 / 20.0;

/// Source units (inches, Z-up) -> Bevy (metres, Y-up). A proper rotation, so
/// mesh winding survives the conversion.
pub const UNITS_TO_METRES: f32 = 0.0254;
pub fn to_bevy(v: apex_math::Vec3) -> Vec3 {
    Vec3::new(v.x, v.z, -v.y) * UNITS_TO_METRES
}

/// Apex's FOV slider value (horizontal degrees at 4:3). 110 is common.
const DEFAULT_APEX_FOV: f32 = 110.0;

fn arg_value(name: &str) -> Option<f64> {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == name {
            return args.next().and_then(|v| v.parse::<f64>().ok());
        }
    }
    None
}

/// Apex FOV slider (horizontal at 4:3) -> vertical FOV in radians, which is
/// what Bevy wants. Apex keeps the vertical FOV and widens horizontally on
/// 16:9, so this matches what Apex shows at any aspect ratio.
fn apex_fov_to_vertical(apex_fov_deg: f32) -> f32 {
    let half_h43 = (apex_fov_deg.clamp(70.0, 120.0) * 0.5).to_radians();
    2.0 * (half_h43.tan() * 0.75).atan()
}

/// `--hz N`: fixed-rate simulation (decoupled from rendering). None = one
/// tick per frame, like Apex.
fn fixed_hz_from_args() -> Option<f64> {
    arg_value("--hz").filter(|v| (10.0..=1000.0).contains(v))
}

/// Default frame cap: 165 fps, a common Apex fps_max and the rate the
/// superglide was practised at (jump→crouch exactly one frame apart).
const DEFAULT_FPS_CAP: f64 = 165.0;

fn fps_cap_from_args() -> Option<f64> {
    if std::env::args().any(|a| a == "--vsync") {
        return None;
    }
    Some(
        arg_value("--fps")
            .filter(|v| (20.0..=1000.0).contains(v))
            .unwrap_or(DEFAULT_FPS_CAP),
    )
}
/// Apex (like Source) turns 0.022° per mouse count × the sensitivity setting.
const DEG_PER_COUNT_PER_SENS: f32 = 0.022;
const DEFAULT_APEX_SENS: f32 = 2.0;

fn main() {
    let fps_cap = fps_cap_from_args();
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "ApexRuntime sandbox".into(),
                resolution: (1600, 900).into(),
                // A cap replaces vsync, like fps_max in game.
                present_mode: if fps_cap.is_some() {
                    bevy::window::PresentMode::AutoNoVsync
                } else {
                    bevy::window::PresentMode::AutoVsync
                },
                ..default()
            }),
            ..default()
        }))
        .insert_resource(FrameLimiter {
            frame: fps_cap.map(|f| std::time::Duration::from_secs_f64(1.0 / f)),
            next: None,
        })
        .add_systems(Last, limit_frame_rate)
        .insert_resource(ClearColor(Color::srgb(0.53, 0.66, 0.80)))
        .insert_resource(Sim::new())
        .insert_resource(view::ViewState::default())
        .add_systems(
            Startup,
            ((setup_world, view::setup_view).chain(), setup_hud),
        )
        .add_systems(
            Update,
            (
                grab_cursor,
                look,
                simulate,
                view::place_camera,
                view::animate_arms,
                view::draw_trail,
                update_hud,
                hotkeys,
                view::view_hotkeys,
            )
                .chain(),
        )
        .run();
}

#[derive(Resource)]
pub(crate) struct Sim {
    pub(crate) world: BrushWorld,
    pub(crate) cfg: MovementConfig,
    pub(crate) clock: FixedClock,
    pub(crate) prev: MovementState,
    pub(crate) cur: MovementState,
    pub(crate) last_report: StepReport,
    pub(crate) last_input: MovementInput,
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    /// A jump press survives until a tick consumes it, so a tap shorter than
    /// one tick is never lost between render frames.
    pub(crate) jump_latch: bool,
    /// Same for crouch: a superglide's crouch tap can be shorter than a tick.
    pub(crate) crouch_latch: bool,
    /// Ticks per second: fixed with `--hz`, else the (smoothed) frame rate.
    pub(crate) tick_rate: f64,
    /// One movement tick per rendered frame (Apex), unless `--hz` was given.
    pub(crate) frame_locked: bool,
    /// A scroll notch arrived since the last tick (forward / jump).
    pub(crate) scroll_fwd: bool,
    pub(crate) scroll_jump: bool,
    pub(crate) auto_sprint: bool,
    /// Last lurch (strength, tick) so the HUD can show it for longer than a tick.
    pub(crate) last_lurch: (f32, u64),
    /// Last traversal / tech event (name, tick) for the HUD.
    pub(crate) last_event: (&'static str, u64),
    /// Last landing: (vertical speed just before it, tick).
    pub(crate) last_landing: (f32, u64),
    /// Current ground contact: (landing tick, speed at landing, jump was
    /// pressed on the landing tick itself = wasted, as in Apex).
    pub(crate) hop_land: Option<(u64, f32, bool)>,
    /// Last bhop: (ground ticks before the jump, speed lost on the ground,
    /// bhop penalty on the jump, wasted press on the landing tick).
    pub(crate) last_hop: Option<(u64, f32, f32, bool)>,
    /// Mouse degrees per count (`--sens`).
    pub(crate) deg_per_count: f32,
    /// Current airborne arc: (start yaw, start heading, start speed, lurches,
    /// view turned so far) and the last finished one, for tap-strafe tuning.
    pub(crate) arc: Option<(f32, f32, f32, u32, f32)>,
    pub(crate) last_arc: Option<(u32, f32, f32, f32, f32)>,
    pub(crate) recording: Option<Recording>,
    pub(crate) hud_visible: bool,
}

struct Recording {
    start: MovementState,
    start_yaw: f32,
    start_pitch: f32,
    rows: Vec<TraceRow>,
    inputs: Vec<MovementInput>,
    dts: Vec<f32>,
}

impl Sim {
    fn new() -> Self {
        let fixed = fixed_hz_from_args();
        let tick_rate = fixed.unwrap_or(144.0);
        let world = proving_ground();
        let s = MovementState::at(SPAWN);
        Self {
            world,
            cfg: MovementConfig::default(),
            clock: FixedClock::new(tick_rate),
            prev: s,
            cur: s,
            last_report: StepReport::default(),
            last_input: MovementInput::default(),
            yaw: 0.0,
            pitch: 0.0,
            jump_latch: false,
            crouch_latch: false,
            tick_rate,
            frame_locked: fixed.is_none(),
            scroll_fwd: false,
            scroll_jump: false,
            auto_sprint: true,
            last_lurch: (0.0, 0),
            last_event: ("-", 0),
            last_landing: (0.0, 0),
            hop_land: None,
            deg_per_count: DEG_PER_COUNT_PER_SENS
                * arg_value("--sens")
                    .filter(|v| (0.05..=20.0).contains(v))
                    .map_or(DEFAULT_APEX_SENS, |v| v as f32),
            arc: None,
            last_arc: None,
            last_hop: None,
            recording: None,
            hud_visible: true,
        }
    }

    /// Render interpolation between the last two ticks (1 when frame-locked:
    /// the newest tick *is* this frame).
    pub(crate) fn alpha(&self) -> f32 {
        if self.frame_locked {
            1.0
        } else {
            self.clock.alpha()
        }
    }
}

/// Optional frame cap (`--fps`): sleeps, then spins the last bit for accuracy.
#[derive(Resource)]
struct FrameLimiter {
    frame: Option<std::time::Duration>,
    next: Option<std::time::Instant>,
}

fn limit_frame_rate(mut lim: ResMut<FrameLimiter>) {
    let Some(frame) = lim.frame else { return };
    let now = std::time::Instant::now();
    let target = match lim.next {
        Some(t) if t > now => t,
        // Fell behind (or first frame): don't try to catch up.
        _ => {
            lim.next = Some(now + frame);
            return;
        }
    };
    let spin = std::time::Duration::from_micros(1500);
    if target - now > spin {
        std::thread::sleep(target - now - spin);
    }
    while std::time::Instant::now() < target {
        std::hint::spin_loop();
    }
    lim.next = Some(target + frame);
}

#[derive(Component)]
pub(crate) struct PlayerCamera;

#[derive(Component)]
struct Hud;

fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    sim: Res<Sim>,
) {
    let grid = images.add(view::grid_texture());
    let palette = [
        Color::srgb(0.42, 0.44, 0.47),
        Color::srgb(0.85, 0.55, 0.30),
        Color::srgb(0.35, 0.60, 0.80),
        Color::srgb(0.55, 0.75, 0.40),
        Color::srgb(0.80, 0.40, 0.45),
    ];
    for (i, brush) in sim.world.brushes.iter().enumerate() {
        let color = if i == 0 {
            palette[0]
        } else {
            palette[1 + i % 4]
        };
        commands.spawn((
            Mesh3d(meshes.add(brush_mesh::brush_to_mesh(brush))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color,
                base_color_texture: Some(grid.clone()),
                perceptual_roughness: 0.9,
                ..default()
            })),
        ));
    }

    commands.spawn((
        DirectionalLight {
            illuminance: 9000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
        bevy::camera::visibility::RenderLayers::from_layers(&[0, view::VIEW_MODEL_LAYER]),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 2500.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-8.0, 10.0, -12.0).looking_at(Vec3::ZERO, Vec3::Y),
        bevy::camera::visibility::RenderLayers::from_layers(&[0, view::VIEW_MODEL_LAYER]),
    ));

    commands.spawn((
        PlayerCamera,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: apex_fov_to_vertical(arg_value("--fov").map_or(DEFAULT_APEX_FOV, |v| v as f32)),
            near: 0.02,
            ..default()
        }),
        Transform::default(),
    ));
}

fn setup_hud(mut commands: Commands) {
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont {
            font_size: bevy::text::FontSize::Px(15.0),
            ..default()
        },
        TextColor(Color::WHITE),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(8.0),
            left: Val::Px(8.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        },
    ));
}

fn grab_cursor(
    mut cursor: Single<&mut CursorOptions, With<PrimaryWindow>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

fn look(
    motion: Res<AccumulatedMouseMotion>,
    cursor: Single<&CursorOptions, With<PrimaryWindow>>,
    mut sim: ResMut<Sim>,
) {
    if cursor.grab_mode == CursorGrabMode::None {
        return;
    }
    let d = motion.delta;
    let k = sim.deg_per_count;
    sim.yaw = apex_math::wrap_degrees(sim.yaw - d.x * k);
    sim.pitch = (sim.pitch + d.y * k).clamp(-89.0, 89.0);
}

fn sample_input(
    keys: &ButtonInput<KeyCode>,
    sim: &Sim,
    fwd_pulse: bool,
    jump_pulse: bool,
) -> MovementInput {
    let axis =
        |pos: KeyCode, neg: KeyCode| (keys.pressed(pos) as i32 - keys.pressed(neg) as i32) as f32;
    let mut buttons = Buttons::NONE;
    buttons.set(
        Buttons::JUMP,
        keys.pressed(KeyCode::Space) || sim.jump_latch || jump_pulse,
    );
    buttons.set(
        Buttons::CROUCH,
        keys.pressed(KeyCode::ControlLeft)
            || keys.pressed(KeyCode::KeyC)
            || keys.pressed(KeyCode::KeyL)
            || sim.crouch_latch,
    );
    buttons.set(
        Buttons::SPRINT,
        sim.auto_sprint || keys.pressed(KeyCode::ShiftLeft),
    );
    let mut forward = axis(KeyCode::KeyW, KeyCode::KeyS);
    if fwd_pulse && forward <= 0.0 {
        forward += 1.0;
    }
    MovementInput {
        forward,
        right: axis(KeyCode::KeyD, KeyCode::KeyA),
        buttons,
        yaw: sim.yaw,
        pitch: sim.pitch,
    }
}

fn simulate(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    mut sim: ResMut<Sim>,
) {
    if keys.just_pressed(KeyCode::Space) {
        sim.jump_latch = true;
    }
    if keys.any_just_pressed([KeyCode::ControlLeft, KeyCode::KeyC, KeyCode::KeyL]) {
        sim.crouch_latch = true;
    }
    for ev in wheel.read() {
        if ev.y > 0.0 {
            sim.scroll_fwd = true;
        } else if ev.y < 0.0 {
            sim.scroll_jump = true;
        }
    }
    let frame = time.delta_secs_f64();
    let (ticks, dt) = if sim.frame_locked {
        if frame <= 0.0 {
            return;
        }
        let dt = frame.clamp(MIN_FRAME_DT, MAX_FRAME_DT);
        sim.tick_rate = sim.tick_rate * 0.95 + 0.05 / dt;
        (1, dt as f32)
    } else {
        let ticks = sim.clock.advance(frame);
        (ticks, sim.clock.dt())
    };
    for _ in 0..ticks {
        // Notches belong to the next tick only (Source: a press + release in
        // one frame sets the button for that usercmd). Two notches in one
        // frame are one press; notches in back-to-back ticks read as held.
        let (fwd_pulse, jump_pulse) = (sim.scroll_fwd, sim.scroll_jump);
        sim.scroll_fwd = false;
        sim.scroll_jump = false;

        let input = sample_input(&keys, &sim, fwd_pulse, jump_pulse);
        let (next, rep) = step_with_report(&sim.cur, &input, &sim.cfg, &sim.world, dt);
        sim.prev = sim.cur;
        sim.cur = next;
        sim.last_report = rep;
        if rep.lurch_k > 0.0 {
            sim.last_lurch = (rep.lurch_k, next.tick);
        }
        let event = if rep.superglide {
            Some("SUPERGLIDE")
        } else if rep.mantle_boost {
            Some("MANTLE BOOST")
        } else if rep.wall_jump {
            Some("wall jump")
        } else if rep.climb_finished {
            Some("climb push")
        } else if rep.climb_started {
            Some("wall climb")
        } else if rep.mantle_finished {
            Some("mantle")
        } else if rep.mantle_started {
            Some("mantle start")
        } else if rep.slide_boosted {
            Some("slide boost")
        } else {
            None
        };
        if let Some(e) = event {
            sim.last_event = (e, next.tick);
        }
        if rep.landed {
            sim.last_landing = (sim.prev.velocity.z, next.tick);
            let wasted = input.buttons.contains(Buttons::JUMP)
                && !sim.last_input.buttons.contains(Buttons::JUMP);
            sim.hop_land = Some((next.tick, next.horizontal_speed(), wasted));
        }
        if rep.jumped && sim.prev.on_ground() {
            if let Some((tick, land_speed, wasted)) = sim.hop_land.take() {
                let before = sim.prev.horizontal_speed();
                sim.last_hop = Some((
                    next.tick - tick - 1,
                    land_speed - before,
                    before - next.horizontal_speed(),
                    wasted,
                ));
            }
        } else if !next.on_ground() {
            sim.hop_land = None;
        }
        // Airborne arc readout (view turn vs velocity turn, lurches, speed).
        let heading = |v: apex_math::Vec3| v.y.atan2(v.x).to_degrees();
        let airborne = !next.on_ground() && next.mantle.is_none() && next.climb.is_none();
        match (sim.arc, airborne) {
            (None, true) => {
                sim.arc = Some((
                    input.yaw,
                    heading(next.velocity),
                    next.horizontal_speed(),
                    0,
                    0.0,
                ));
            }
            (Some((y0, h0, s0, n, _)), true) => {
                let lurched = u32::from(rep.lurch_k > 0.0);
                let view = apex_math::wrap_degrees(input.yaw - y0);
                sim.arc = Some((y0, h0, s0, n + lurched, view));
            }
            (Some((_, h0, s0, n, view)), false) => {
                let turned = apex_math::wrap_degrees(heading(sim.prev.velocity) - h0);
                sim.last_arc = Some((n, view, turned, s0, sim.prev.horizontal_speed()));
                sim.arc = None;
            }
            (None, false) => {}
        }
        sim.last_input = input;
        sim.jump_latch = false;
        sim.crouch_latch = false;
        if let Some(rec) = sim.recording.as_mut() {
            rec.rows.push(TraceRow::new(&next, &input, &rep));
            rec.inputs.push(input);
            rec.dts.push(dt);
        }
    }
}

fn update_hud(sim: Res<Sim>, mut hud: Single<(&mut Text, &mut Visibility), With<Hud>>) {
    let (text, vis) = &mut *hud;
    **vis = if sim.hud_visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !sim.hud_visible {
        return;
    }
    let s = &sim.cur;
    let r = &sim.last_report;
    let i = &sim.last_input;
    let rec = match &sim.recording {
        Some(rec) => format!("REC {} ticks (F5 to stop)", rec.rows.len()),
        None => "F5 record".to_owned(),
    };
    text.0 = format!(
        "tick      {}\n\
         mode      {}{}\n\
         pos       {:8.2} {:8.2} {:8.2}\n\
         vel       {:8.2} {:8.2} {:8.2}\n\
         speed h   {:8.2} u/s   ({:.2} m/s)\n\
         speed v   {:8.2} u/s\n\
         input     fwd {:+.0} right {:+.0} buttons {:03b}\n\
         view      yaw {:7.2}  pitch {:6.2}\n\
         wish      {:6.1} u/s   accel +{:.2}  friction -{:.2}\n\
         air {:.3}s ground {:.3}s {}\n\
         last lurch k {:.2} ({} ticks ago)   slide boost {}\n\
         always-sprint {} (F2)   scroll: up = W tap, down = jump\n\
         F3 1st/3rd person   F4 camera feel   F6 trail   F7 jump fatigue {}\n\
         last event: {} ({} ticks ago)   {}\n\
         last hop: {}\n\
         climb space: {}\n\
         last air arc: {}\n\
         {}\n\
         [Apex S21 rules where measured; see sheets/constants.tsv]",
        s.tick,
        s.mode_name(),
        if s.crouched { " (crouched)" } else { "" },
        s.position.x,
        s.position.y,
        s.position.z,
        s.velocity.x,
        s.velocity.y,
        s.velocity.z,
        s.horizontal_speed(),
        s.horizontal_speed() * UNITS_TO_METRES,
        s.velocity.z,
        i.forward,
        i.right,
        i.buttons.0,
        sim.yaw,
        sim.pitch,
        r.wish_speed,
        r.accel_added,
        r.friction_drop,
        s.time_in_air,
        s.time_on_ground,
        if r.hit_wall { "WALL" } else { "" },
        sim.last_lurch.0,
        s.tick.saturating_sub(sim.last_lurch.1),
        if s.time_since_slide_boost >= sim.cfg.slide_boost_cooldown {
            "ready".to_owned()
        } else {
            format!(
                "in {:.1}s",
                sim.cfg.slide_boost_cooldown - s.time_since_slide_boost
            )
        },
        if sim.auto_sprint { "ON" } else { "OFF" },
        if sim.cfg.jump_fatigue_min_frac < 1.0 {
            "ON"
        } else {
            "OFF"
        },
        sim.last_event.0,
        s.tick.saturating_sub(sim.last_event.1),
        if sim.frame_locked {
            format!(
                "1 tick per frame, {:.0} fps (--fps N to cap)",
                sim.tick_rate
            )
        } else {
            format!("fixed {} Hz (--hz)", sim.tick_rate)
        },
        match sim.last_hop {
            None => "-".to_owned(),
            Some((ticks, ground, penalty, wasted)) => format!(
                "{ticks} ground ticks before the jump, -{ground:.1} friction/slide, \
                 -{penalty:.1} bhop penalty{}",
                if wasted {
                    "  (a press ON the landing tick was too early)"
                } else {
                    ""
                }
            ),
        },
        climb_space_text(s, &sim.cfg),
        match sim.last_arc {
            None => "-".to_owned(),
            Some((n, view, turned, s0, s1)) => format!(
                "{n} lurches, view turned {view:+.0}°, velocity turned {turned:+.0}°, \
                 speed {s0:.0} -> {s1:.0}   (sens {:.2})",
                sim.deg_per_count / DEG_PER_COUNT_PER_SENS
            ),
        },
        rec,
    );
}

/// Height above the climb-space baseline and the zone a wall jump would use.
fn climb_space_text(s: &MovementState, cfg: &MovementConfig) -> String {
    let z = s.position.z - s.climb_base;
    let zone = if z >= cfg.climb_space_height {
        "above the cutoff (no climbing)"
    } else if z <= cfg.climb_mini_zone_top {
        "MINI (mini-bounce)"
    } else if z <= cfg.climb_green_zone_top {
        "GREEN (wall bounce)"
    } else {
        "neutral (wall push)"
    };
    format!("{z:+6.1} u above baseline   zone {zone}")
}

fn hotkeys(keys: Res<ButtonInput<KeyCode>>, mut sim: ResMut<Sim>) {
    if keys.just_pressed(KeyCode::F1) {
        sim.hud_visible = !sim.hud_visible;
    }
    if keys.just_pressed(KeyCode::F2) {
        sim.auto_sprint = !sim.auto_sprint;
    }
    if keys.just_pressed(KeyCode::F7) {
        // Compare with / without jump fatigue (measured on S21 Flowstate;
        // live Apex may differ).
        let measured = MovementConfig::default().jump_fatigue_min_frac;
        sim.cfg.jump_fatigue_min_frac = if sim.cfg.jump_fatigue_min_frac < 1.0 {
            1.0
        } else {
            measured
        };
    }
    if keys.just_pressed(KeyCode::KeyR) {
        let s = MovementState::at(SPAWN);
        sim.prev = s;
        sim.cur = s;
    }
    if keys.just_pressed(KeyCode::F5) {
        match sim.recording.take() {
            None => {
                let (start, start_yaw, start_pitch) = (sim.cur, sim.yaw, sim.pitch);
                sim.recording = Some(Recording {
                    start,
                    start_yaw,
                    start_pitch,
                    rows: Vec::new(),
                    inputs: Vec::new(),
                    dts: Vec::new(),
                });
            }
            Some(rec) => match save_recording(&rec, sim.tick_rate) {
                Ok(path) => info!("saved recording to {}", path.display()),
                Err(e) => error!("could not save recording: {e}"),
            },
        }
    }
}

/// Writes `traces/sandbox-<n>.csv` (what happened) and
/// `traces/sandbox-<n>.input.json` (an input script that reproduces it with
/// `apex-replay`, as long as the recording started grounded or at rest).
fn save_recording(rec: &Recording, rec_tick_rate: f64) -> std::io::Result<PathBuf> {
    let dir = PathBuf::from("traces");
    std::fs::create_dir_all(&dir)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let csv = dir.join(format!("sandbox-{stamp}.csv"));
    write_csv(
        &rec.rows,
        std::io::BufWriter::new(std::fs::File::create(&csv)?),
    )?;

    let mut events = Vec::new();
    let mut prev: Option<(MovementInput, f32)> = None;
    for (tick, (input, &dt)) in rec.inputs.iter().zip(&rec.dts).enumerate() {
        if prev == Some((*input, dt)) {
            continue;
        }
        let mut e = InputEvent {
            tick: tick as u64,
            // Frame-locked recordings carry each frame's dt.
            dt: Some(dt),
            ..InputEvent::default()
        };
        e.forward = Some(input.forward);
        e.right = Some(input.right);
        e.yaw = Some(input.yaw);
        e.pitch = Some(input.pitch);
        for (b, name) in [
            (Buttons::JUMP, "jump"),
            (Buttons::CROUCH, "crouch"),
            (Buttons::SPRINT, "sprint"),
        ] {
            if input.buttons.contains(b) {
                e.press.push(name.into());
            } else {
                e.release.push(name.into());
            }
        }
        events.push(e);
        prev = Some((*input, dt));
    }
    let script = InputScript {
        name: format!("sandbox-{stamp}"),
        description: "Recorded in apex-sandbox.".into(),
        tick_rate: rec_tick_rate,
        ticks: rec.inputs.len() as u64,
        start: ScriptStart {
            position: rec.start.position.to_array(),
            velocity: rec.start.velocity.to_array(),
            yaw: rec.start_yaw,
            pitch: rec.start_pitch,
        },
        events,
    };
    let json = dir.join(format!("sandbox-{stamp}.input.json"));
    std::fs::write(
        &json,
        serde_json::to_vec_pretty(&script).map_err(std::io::Error::other)?,
    )?;
    Ok(csv)
}
