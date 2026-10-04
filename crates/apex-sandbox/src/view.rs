//! Presentation only: camera (first/third person, optional "feel"), a
//! procedural first-person view model (arms), a hull-sized body for third
//! person, a movement trail, and the measuring grid texture. Nothing here
//! feeds back into the simulation.
//!
//! Keys: F3 first/third person · F4 camera feel on/off · F6 movement trail.

use std::collections::VecDeque;
use std::f32::consts::{PI, TAU};

use apex_collision::CollisionWorld;
use apex_core::MovementState;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::RenderLayers;
use bevy::image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor};
use bevy::light::NotShadowCaster;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::{PlayerCamera, Sim, UNITS_TO_METRES, to_bevy};

/// Render layer of the view model (arms): drawn by its own camera on top of
/// the world, so the arms never clip into walls.
pub const VIEW_MODEL_LAYER: usize = 1;
/// Seconds of positions kept for the trail.
const TRAIL_SECONDS: f32 = 3.0;

#[derive(Resource)]
pub struct ViewState {
    pub third_person: bool,
    pub feel: bool,
    pub trail: bool,
    bob_phase: f32,
    dip: f32,
    dip_vel: f32,
    roll: f32,
    trail_points: VecDeque<Vec3>,
    last_landing_tick: u64,
    eye_frac: f32,
    /// Camera-only mantle smoothing: offset from the hull eye that decays
    /// after the mantle ends, and the last mantle camera position.
    mantle_cam_offset: Vec3,
    last_mantle_cam: Option<Vec3>,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            third_person: false,
            feel: true,
            trail: true,
            bob_phase: 0.0,
            dip: 0.0,
            dip_vel: 0.0,
            roll: 0.0,
            trail_points: VecDeque::new(),
            last_landing_tick: 0,
            eye_frac: 0.0,
            mantle_cam_offset: Vec3::ZERO,
            last_mantle_cam: None,
        }
    }
}

#[derive(Component)]
pub struct ViewModelCamera;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Arm {
    Left,
    Right,
}

#[derive(Component)]
pub struct Body;

#[derive(Component)]
pub struct BodyNose;

/// Grid texture for one 64 u tile: a thin line every 16 u, a bold one on the
/// tile edge (64 u), soft-edged to limit shimmering. White-ish so the
/// material colour tints it.
pub fn grid_texture() -> Image {
    const SIZE: usize = 256;
    let px_per_unit = SIZE as f32 / 64.0;
    let mut data = vec![0u8; SIZE * SIZE * 4];
    let line = |p: usize| -> f32 {
        // distance in units to the nearest 16 u line, and whether it is a 64 u line
        let u = p as f32 / px_per_unit;
        let d16 = (u - (u / 16.0).round() * 16.0).abs();
        let d64 = (u - (u / 64.0).round() * 64.0).abs();
        let major = (1.0 - (d64 - 0.6).max(0.0) / 0.5).clamp(0.0, 1.0);
        let minor = (1.0 - (d16 - 0.25).max(0.0) / 0.4).clamp(0.0, 1.0) * 0.45;
        major.max(minor)
    };
    for y in 0..SIZE {
        for x in 0..SIZE {
            let k = line(x).max(line(y));
            let base = 225.0;
            let v = (base * (1.0 - 0.55 * k)) as u8;
            let i = (y * SIZE + x) * 4;
            data[i..i + 4].copy_from_slice(&[v, v, v, 255]);
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: SIZE as u32,
            height: SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    let mut sampler = ImageSamplerDescriptor::linear();
    sampler.set_address_mode(ImageAddressMode::Repeat);
    sampler.set_anisotropic_filter(16);
    image.sampler = ImageSampler::Descriptor(sampler);
    image
}

/// Fog, the view-model camera and arms (children of the player camera), and
/// the third-person body.
pub fn setup_view(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cam: Single<Entity, With<PlayerCamera>>,
    clear: Res<ClearColor>,
) {
    let cam = *cam;
    commands.entity(cam).insert(DistanceFog {
        color: clear.0,
        falloff: FogFalloff::Linear {
            start: 35.0,
            end: 140.0,
        },
        ..default()
    });

    let layer = RenderLayers::layer(VIEW_MODEL_LAYER);
    let sleeve = materials.add(StandardMaterial {
        base_color: Color::srgb(0.16, 0.18, 0.22),
        perceptual_roughness: 0.8,
        ..default()
    });
    let glove = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.42, 0.18),
        perceptual_roughness: 0.6,
        ..default()
    });
    let forearm = meshes.add(Cuboid::new(0.05, 0.05, 0.30));
    let hand = meshes.add(Cuboid::new(0.062, 0.03, 0.075));
    let fingers = meshes.add(Cuboid::new(0.056, 0.022, 0.042));

    commands.entity(cam).with_children(|c| {
        c.spawn((
            ViewModelCamera,
            Camera3d::default(),
            Camera {
                order: 1,
                ..default()
            },
            Projection::Perspective(PerspectiveProjection {
                fov: 68.0_f32.to_radians(),
                near: 0.01,
                ..default()
            }),
            layer.clone(),
        ));
        for side in [Arm::Left, Arm::Right] {
            c.spawn((side, Transform::default(), Visibility::default()))
                .with_children(|a| {
                    a.spawn((
                        Mesh3d(forearm.clone()),
                        MeshMaterial3d(sleeve.clone()),
                        Transform::from_xyz(0.0, 0.0, 0.08),
                        layer.clone(),
                        NotShadowCaster,
                    ));
                    a.spawn((
                        Mesh3d(hand.clone()),
                        MeshMaterial3d(glove.clone()),
                        Transform::from_xyz(0.0, 0.0, -0.105),
                        layer.clone(),
                        NotShadowCaster,
                    ));
                    a.spawn((
                        Mesh3d(fingers.clone()),
                        MeshMaterial3d(glove.clone()),
                        Transform::from_xyz(0.0, -0.01, -0.16)
                            .with_rotation(Quat::from_rotation_x(-0.5)),
                        layer.clone(),
                        NotShadowCaster,
                    ));
                });
        }
    });

    // Third-person body: the collision hull, see-through, plus a facing marker.
    let hull = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    commands.spawn((
        Body,
        Mesh3d(hull),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.95, 0.55, 0.2, 0.35),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
    ));
    commands.spawn((
        BodyNose,
        Mesh3d(meshes.add(Cuboid::new(0.12, 0.08, 0.35))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.1, 0.1, 0.1),
            unlit: true,
            ..default()
        })),
        Transform::default(),
        Visibility::Hidden,
    ));
}

pub fn view_hotkeys(keys: Res<ButtonInput<KeyCode>>, mut view: ResMut<ViewState>) {
    if keys.just_pressed(KeyCode::F3) {
        view.third_person = !view.third_person;
    }
    if keys.just_pressed(KeyCode::F4) {
        view.feel = !view.feel;
    }
    if keys.just_pressed(KeyCode::F6) {
        view.trail = !view.trail;
        view.trail_points.clear();
    }
}

fn eye_height(sim: &Sim, frac: f32) -> f32 {
    sim.cfg.stand_eye_height + (sim.cfg.crouch_eye_height - sim.cfg.stand_eye_height) * frac
}

/// Interpolated feet position between the last two ticks.
fn feet(sim: &Sim) -> apex_math::Vec3 {
    sim.prev.position.lerp(sim.cur.position, sim.alpha())
}

fn view_rotation(yaw: f32, pitch: f32, roll: f32) -> Quat {
    // Source yaw 0 looks down +X (Bevy +X); Bevy cameras look down -Z.
    Quat::from_euler(
        EulerRot::YXZ,
        (yaw - 90.0).to_radians(),
        (-pitch).to_radians(),
        roll.to_radians(),
    )
}

/// Camera placement: first person with optional feel, or third person.
#[allow(clippy::type_complexity)]
pub fn place_camera(
    time: Res<Time>,
    sim: Res<Sim>,
    mut view: ResMut<ViewState>,
    mut cam: Single<&mut Transform, (With<PlayerCamera>, Without<Body>, Without<BodyNose>)>,
    mut vm_cam: Single<&mut Camera, With<ViewModelCamera>>,
    mut body: Single<
        (&mut Transform, &mut Visibility),
        (With<Body>, Without<PlayerCamera>, Without<BodyNose>),
    >,
    mut nose: Single<
        (&mut Transform, &mut Visibility),
        (With<BodyNose>, Without<PlayerCamera>, Without<Body>),
    >,
) {
    let dt = time.delta_secs().min(0.1);
    let s = &sim.cur;
    let speed_h = s.horizontal_speed();

    // Eye height follows the crouch smoothly (crouch_frac on the ground,
    // instant in the air like the hull).
    let target_frac = if s.crouched { 1.0 } else { s.crouch_frac };
    view.eye_frac += (target_frac - view.eye_frac) * (1.0 - (-14.0 * dt).exp());

    // --- feel ---
    let mut bob = 0.0;
    let mut roll_target = 0.0;
    if view.feel {
        let on_foot = s.on_ground() && !s.sliding && s.mantle.is_none();
        if on_foot && speed_h > 20.0 {
            view.bob_phase = (view.bob_phase + speed_h * dt / 70.0 * PI) % TAU;
            bob = (2.0 * view.bob_phase).sin() * 0.9 * (speed_h / 299.0).min(1.2);
        }
        roll_target = -sim.last_input.right * 1.2;
        if s.sliding {
            roll_target += 3.0;
        }
        // Landing dip: a damped spring kicked by the impact speed.
        if sim.last_landing.1 != view.last_landing_tick {
            view.last_landing_tick = sim.last_landing.1;
            view.dip_vel -= (sim.last_landing.0.abs() / 6.0).min(160.0);
        }
        let (k, c) = (220.0, 26.0);
        let acc = -k * view.dip - c * view.dip_vel;
        view.dip_vel += acc * dt;
        view.dip += view.dip_vel * dt;
    } else {
        view.dip = 0.0;
        view.dip_vel = 0.0;
    }
    view.roll += (roll_target - view.roll) * (1.0 - (-10.0 * dt).exp());

    let feet = feet(&sim);
    let eye_units =
        feet + apex_math::Vec3::new(0.0, 0.0, eye_height(&sim, view.eye_frac) + bob + view.dip);
    let mut eye = to_bevy(eye_units);
    let mut pitch = sim.pitch;

    // Mantle camera (presentation only, physics untouched): the recorded hull
    // barely moves forward during a mantle and exits at full speed; Apex hides
    // that with an animated traversal camera. Lead the view over the ledge on
    // a smooth arc, dip it toward the ledge, then ease into the normal view.
    if let Some(m) = s.mantle {
        let u = if m.duration > 0.0 {
            (m.t / m.duration).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let smooth = |x: f32| x * x * (3.0 - 2.0 * x);
        let hull_over = smooth(((u - 0.2) / 0.8).clamp(0.0, 1.0));
        let lead = smooth(u);
        let span_h = (m.end - m.start).horizontal();
        // With W held the exit is at full speed: let the camera get a head
        // start in the last part, so the exit reads as acceleration, not a jolt.
        let head_start = if sim.last_input.forward > 0.0 {
            12.0
        } else {
            0.0
        };
        let late = smooth(((u - 0.55) / 0.45).clamp(0.0, 1.0));
        let ahead = span_h * (lead - hull_over) + m.dir * (head_start * late);
        eye += to_bevy(ahead) - to_bevy(apex_math::Vec3::ZERO);
        pitch += 7.0 * (PI * u).sin();
        view.last_mantle_cam = Some(eye);
    } else if let Some(last) = view.last_mantle_cam.take() {
        view.mantle_cam_offset = last - eye;
    }
    view.mantle_cam_offset *= (-dt / 0.12).exp();
    if s.mantle.is_none() {
        eye += view.mantle_cam_offset;
    }
    let rot = view_rotation(sim.yaw, pitch, if view.feel { view.roll } else { 0.0 });

    if view.third_person {
        // Behind and above, pulled in if a wall is in the way.
        let back = rot * Vec3::Z; // Bevy camera looks down -Z
        let desired_units = eye_units
            + apex_math::Vec3::new(back.x, -back.z, back.y) * (130.0 / 1.0)
            + apex_math::Vec3::new(0.0, 0.0, 18.0);
        let tr = sim.world.trace_ray(eye_units, desired_units);
        let dist = (desired_units - eye_units).length();
        let clear = (tr.fraction * dist - 8.0).max(10.0) / dist;
        let cam_units = eye_units.lerp(desired_units, clear.min(1.0));
        cam.translation = to_bevy(cam_units);
        cam.look_at(eye, Vec3::Y);
        vm_cam.is_active = false;
    } else {
        cam.translation = eye;
        cam.rotation = rot;
        vm_cam.is_active = true;
    }

    // Body = the collision hull, centred on it.
    let hull = sim.cfg.hull(s.crouched);
    let size = to_bevy_size(hull.maxs - hull.mins);
    let centre = feet + (hull.mins + hull.maxs) * 0.5;
    let (bt, bv) = &mut *body;
    bt.translation = to_bevy(centre);
    bt.scale = size;
    **bv = if view.third_person {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    let (nt, nv) = &mut *nose;
    let fwd = view_rotation(sim.yaw, 0.0, 0.0) * Vec3::NEG_Z;
    nt.translation =
        to_bevy(feet + apex_math::Vec3::new(0.0, 0.0, eye_height(&sim, view.eye_frac))) + fwd * 0.3;
    nt.rotation = view_rotation(sim.yaw, sim.pitch, 0.0);
    **nv = if view.third_person {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

fn to_bevy_size(v: apex_math::Vec3) -> Vec3 {
    Vec3::new(v.x, v.z, v.y) * UNITS_TO_METRES
}

/// A pose for one arm, in camera space (metres / radians).
#[derive(Clone, Copy)]
struct Pose {
    pos: Vec3,
    pitch: f32,
    yaw: f32,
    roll: f32,
}

impl Pose {
    fn rest(side: f32) -> Self {
        Self {
            pos: Vec3::new(0.16 * side, -0.15, -0.36),
            pitch: 0.18,
            yaw: -0.14 * side,
            roll: 0.0,
        }
    }
}

fn arm_target(side: f32, s: &MovementState, sim: &Sim, phase: f32, now_tick: u64) -> Pose {
    let mut p = Pose::rest(side);
    let ticks_since = |t: u64| now_tick.saturating_sub(t) as f32 / sim.tick_rate as f32;

    // Short push-off after a wall jump / superglide / mantle boost.
    let (event, event_tick) = sim.last_event;
    let push = matches!(event, "wall jump" | "SUPERGLIDE" | "MANTLE BOOST")
        && ticks_since(event_tick) < 0.25;

    if let Some(m) = s.mantle {
        let u = if m.duration > 0.0 {
            m.t / m.duration
        } else {
            1.0
        };
        // Reach up to the ledge, then push the body up and over it.
        let reach = (u / 0.3).min(1.0);
        let pushdown = ((u - 0.3) / 0.7).clamp(0.0, 1.0);
        p.pos = Vec3::new(
            0.16 * side,
            -0.20 + 0.24 * reach - 0.30 * pushdown,
            -0.44 + 0.08 * pushdown,
        );
        p.pitch = -0.2 - 0.9 * pushdown;
        p.yaw = 0.0;
    } else if let Some(c) = s.climb {
        // Hand over hand.
        let ph = c.t * TAU * 2.2 + if side > 0.0 { 0.0 } else { PI };
        let up = ph.sin().max(0.0);
        p.pos = Vec3::new(0.15 * side, -0.08 + 0.18 * up, -0.40);
        p.pitch = 0.9 + 0.3 * up;
        p.yaw = 0.0;
    } else if push {
        let k = 1.0 - ticks_since(event_tick) / 0.25;
        p.pos += Vec3::new(0.05 * side * k, 0.04 * k, -0.22 * k);
        p.pitch -= 0.3 * k;
    } else if s.sliding {
        p.pos += Vec3::new(0.04 * side, -0.08, 0.02);
        p.roll = 0.35 * side;
        p.pitch -= 0.15;
    } else if !s.on_ground() {
        p.pos += Vec3::new(0.03 * side, 0.03, -0.02);
        p.pitch += 0.12;
    } else {
        let speed = s.horizontal_speed();
        let k = (speed / 299.0).min(1.2);
        let sprinting =
            speed > 210.0 && sim.last_input.buttons.contains(apex_core::Buttons::SPRINT);
        let swing = (phase + if side > 0.0 { 0.0 } else { PI }).sin();
        if sprinting {
            p.pos += Vec3::new(0.0, -0.05 + 0.015 * swing.abs(), 0.06 * swing);
            p.pitch += 0.25 * swing;
            p.roll = 0.12 * side;
        } else {
            p.pos += Vec3::new(
                0.008 * phase.cos() * k,
                0.012 * (2.0 * phase).sin() * k,
                0.0,
            );
        }
        if s.crouched || s.crouch_frac > 0.5 {
            p.pos.y -= 0.03;
        }
    }
    p
}

pub fn animate_arms(
    time: Res<Time>,
    sim: Res<Sim>,
    view: Res<ViewState>,
    mut arms: Query<(&Arm, &mut Transform)>,
) {
    let dt = time.delta_secs().min(0.1);
    let a = 1.0 - (-18.0 * dt).exp();
    for (arm, mut t) in &mut arms {
        let side = if *arm == Arm::Right { 1.0 } else { -1.0 };
        let target = arm_target(side, &sim.cur, &sim, view.bob_phase, sim.cur.tick);
        let rot = Quat::from_euler(EulerRot::YXZ, target.yaw, target.pitch, target.roll);
        t.translation = t.translation.lerp(target.pos, a);
        t.rotation = t.rotation.slerp(rot, a);
    }
}

/// Last few seconds of feet positions, drawn as a line (F6).
pub fn draw_trail(sim: Res<Sim>, mut view: ResMut<ViewState>, mut gizmos: Gizmos) {
    if !view.trail {
        return;
    }
    let p = to_bevy(sim.cur.position) + Vec3::Y * 0.02;
    if view
        .trail_points
        .back()
        .is_none_or(|b| b.distance(p) > 0.02)
    {
        view.trail_points.push_back(p);
    }
    let keep = (TRAIL_SECONDS * 120.0) as usize;
    while view.trail_points.len() > keep {
        view.trail_points.pop_front();
    }
    gizmos.linestrip(
        view.trail_points.iter().copied(),
        Color::srgb(1.0, 0.85, 0.1),
    );
}
