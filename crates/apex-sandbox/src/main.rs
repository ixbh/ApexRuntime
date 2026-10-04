//! apex-sandbox — first-person test bed for apex-core.
//!
//! Bevy is a *host* here, exactly like a future IW4L or Unity adapter:
//! it samples input, drives `apex_core::step` on a fixed clock, and renders
//! the resulting state. Nothing in the movement depends on Bevy.
//!
//! Controls: mouse look · WASD · Space jump · Ctrl/C/L crouch · Shift sprint
//!           scroll up = tap W · scroll down = tap jump (Apex-style binds)
//!           F1 HUD · F2 always-sprint · F5 start/stop recording · R respawn
//!           Esc release mouse
//! Options:  --hz N   simulation ticks per second (default 60; e.g. 165)
//!
//! Scroll notches become one-tick presses with a release tick between them,
//! so every notch is a *new* press (what lurch / tap-strafing keys off).

mod brush_mesh;

use std::path::PathBuf;

use apex_collision::BrushWorld;
use apex_collision::maps::{SPAWN, proving_ground};
use apex_core::{
    Buttons, FixedClock, MovementConfig, MovementInput, MovementState, StepReport, step_with_report,
};
use apex_telemetry::{InputEvent, InputScript, ScriptStart, TraceRow, write_csv};
use bevy::input::mouse::{AccumulatedMouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

/// Most scroll notches queued at once (a fast flick should not keep pressing for seconds).
const MAX_QUEUED_NOTCHES: u32 = 8;

/// Source units (inches, Z-up) -> Bevy (metres, Y-up). A proper rotation, so
/// mesh winding survives the conversion.
pub const UNITS_TO_METRES: f32 = 0.0254;
pub fn to_bevy(v: apex_math::Vec3) -> Vec3 {
    Vec3::new(v.x, v.z, -v.y) * UNITS_TO_METRES
}

/// Default simulation rate. Apex runs movement once per client frame, so
/// frame-tight tech (superglide: crouch exactly one tick after jump) depends
/// on it: run with `--hz 165` to practise at 165 fps timing.
const DEFAULT_TICK_RATE: f64 = 60.0;

fn tick_rate_from_args() -> f64 {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--hz"
            && let Some(v) = args.next().and_then(|v| v.parse::<f64>().ok())
            && (10.0..=1000.0).contains(&v)
        {
            return v;
        }
    }
    DEFAULT_TICK_RATE
}
const MOUSE_DEG_PER_COUNT: f32 = 0.022 * 2.0; // Source m_yaw 0.022 × sensitivity 2

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "ApexRuntime sandbox".into(),
                resolution: (1600, 900).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.53, 0.66, 0.80)))
        .insert_resource(Sim::new())
        .add_systems(Startup, (setup_world, setup_hud))
        .add_systems(
            Update,
            (
                grab_cursor,
                look,
                simulate,
                place_camera,
                update_hud,
                hotkeys,
            )
                .chain(),
        )
        .run();
}

#[derive(Resource)]
struct Sim {
    world: BrushWorld,
    cfg: MovementConfig,
    clock: FixedClock,
    prev: MovementState,
    cur: MovementState,
    last_report: StepReport,
    last_input: MovementInput,
    yaw: f32,
    pitch: f32,
    /// A jump press survives until a tick consumes it, so a tap shorter than
    /// one tick is never lost between render frames.
    jump_latch: bool,
    /// Same for crouch: a superglide's crouch tap can be shorter than a tick.
    crouch_latch: bool,
    tick_rate: f64,
    /// Scroll-wheel notches waiting to be turned into one-tick presses.
    scroll_fwd_pending: u32,
    scroll_jump_pending: u32,
    /// The previous tick was a scroll pulse, so this one must be a release.
    fwd_pulse_last: bool,
    jump_pulse_last: bool,
    auto_sprint: bool,
    /// Last lurch (strength, tick) so the HUD can show it for longer than a tick.
    last_lurch: (f32, u64),
    /// Last traversal / tech event (name, tick) for the HUD.
    last_event: (&'static str, u64),
    recording: Option<Recording>,
    hud_visible: bool,
}

struct Recording {
    start: MovementState,
    start_yaw: f32,
    start_pitch: f32,
    rows: Vec<TraceRow>,
    inputs: Vec<MovementInput>,
}

impl Sim {
    fn new() -> Self {
        let tick_rate = tick_rate_from_args();
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
            scroll_fwd_pending: 0,
            scroll_jump_pending: 0,
            fwd_pulse_last: false,
            jump_pulse_last: false,
            auto_sprint: true,
            last_lurch: (0.0, 0),
            last_event: ("-", 0),
            recording: None,
            hud_visible: true,
        }
    }
}

#[derive(Component)]
struct PlayerCamera;

#[derive(Component)]
struct Hud;

fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    sim: Res<Sim>,
) {
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
                perceptual_roughness: 0.9,
                ..default()
            })),
        ));
    }

    // Floor grid every 64u so speed is visible.
    let line = materials.add(StandardMaterial {
        base_color: Color::srgb(0.30, 0.31, 0.33),
        unlit: true,
        ..default()
    });
    let len = 4000.0 * UNITS_TO_METRES;
    let strip_x = meshes.add(Cuboid::new(len, 0.002, 0.02));
    let strip_z = meshes.add(Cuboid::new(0.02, 0.002, len));
    for k in -31..=31 {
        let o = k as f32 * 64.0 * UNITS_TO_METRES;
        commands.spawn((
            Mesh3d(strip_x.clone()),
            MeshMaterial3d(line.clone()),
            Transform::from_xyz(0.0, 0.001, o),
        ));
        commands.spawn((
            Mesh3d(strip_z.clone()),
            MeshMaterial3d(line.clone()),
            Transform::from_xyz(o, 0.001, 0.0),
        ));
    }

    commands.spawn((
        DirectionalLight {
            illuminance: 9000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 2500.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-8.0, 10.0, -12.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        PlayerCamera,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 70.0_f32.to_radians(),
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
    sim.yaw = apex_math::wrap_degrees(sim.yaw - d.x * MOUSE_DEG_PER_COUNT);
    sim.pitch = (sim.pitch + d.y * MOUSE_DEG_PER_COUNT).clamp(-89.0, 89.0);
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
        // Line units are notches; pixel units (touchpads) count as one notch.
        let notches = match ev.unit {
            MouseScrollUnit::Line => ev.y.abs().round().max(1.0) as u32,
            MouseScrollUnit::Pixel => 1,
        };
        if ev.y > 0.0 {
            sim.scroll_fwd_pending = (sim.scroll_fwd_pending + notches).min(MAX_QUEUED_NOTCHES);
        } else if ev.y < 0.0 {
            sim.scroll_jump_pending = (sim.scroll_jump_pending + notches).min(MAX_QUEUED_NOTCHES);
        }
    }
    let ticks = sim.clock.advance(time.delta_secs_f64());
    for _ in 0..ticks {
        let fwd_pulse = sim.scroll_fwd_pending > 0 && !sim.fwd_pulse_last;
        if fwd_pulse {
            sim.scroll_fwd_pending -= 1;
        }
        sim.fwd_pulse_last = fwd_pulse;
        let jump_pulse = sim.scroll_jump_pending > 0 && !sim.jump_pulse_last;
        if jump_pulse {
            sim.scroll_jump_pending -= 1;
        }
        sim.jump_pulse_last = jump_pulse;

        let input = sample_input(&keys, &sim, fwd_pulse, jump_pulse);
        let dt = sim.clock.dt();
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
        sim.last_input = input;
        sim.jump_latch = false;
        sim.crouch_latch = false;
        if let Some(rec) = sim.recording.as_mut() {
            rec.rows.push(TraceRow::new(&next, &input, &rep));
            rec.inputs.push(input);
        }
    }
}

fn eye_height(sim: &Sim, s: &MovementState) -> f32 {
    if s.crouched {
        sim.cfg.crouch_eye_height
    } else {
        sim.cfg.stand_eye_height
    }
}

fn place_camera(sim: Res<Sim>, mut cam: Single<&mut Transform, With<PlayerCamera>>) {
    let a = sim.clock.alpha();
    let eye = |s: &MovementState| s.position + apex_math::Vec3::new(0.0, 0.0, eye_height(&sim, s));
    let pos = eye(&sim.prev).lerp(eye(&sim.cur), a);
    cam.translation = to_bevy(pos);
    // Source yaw 0 looks down +X (Bevy +X); Bevy cameras look down -Z.
    cam.rotation = Quat::from_euler(
        EulerRot::YXZ,
        (sim.yaw - 90.0).to_radians(),
        (-sim.pitch).to_radians(),
        0.0,
    );
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
         last event: {} ({} ticks ago)   sim {} Hz (--hz)\n\
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
        sim.last_event.0,
        s.tick.saturating_sub(sim.last_event.1),
        sim.tick_rate,
        rec,
    );
}

fn hotkeys(keys: Res<ButtonInput<KeyCode>>, mut sim: ResMut<Sim>) {
    if keys.just_pressed(KeyCode::F1) {
        sim.hud_visible = !sim.hud_visible;
    }
    if keys.just_pressed(KeyCode::F2) {
        sim.auto_sprint = !sim.auto_sprint;
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
    let mut prev: Option<MovementInput> = None;
    for (tick, input) in rec.inputs.iter().enumerate() {
        if prev.as_ref() == Some(input) {
            continue;
        }
        let mut e = InputEvent {
            tick: tick as u64,
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
        prev = Some(*input);
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
