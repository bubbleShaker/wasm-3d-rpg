//! 光・空・フォグなどの環境演出。

use std::f32::consts::TAU;

use bevy::{
    light::{CascadeShadowConfigBuilder, DirectionalLightShadowMap, NotShadowCaster},
    prelude::*,
};

use crate::{
    assets::GameAssets,
    camera::MainCamera,
    field::Water,
    player::Player,
    util::{LpMesh, Rng},
    zone::{CurrentZone, ZoneEntered, ZoneEntity, ZoneId},
};

#[derive(Component)]
pub struct Sun;

#[derive(Component)]
struct SkyDome;

#[derive(Component)]
struct Clouds;

#[derive(Component)]
struct SunDisc;

/// フィールドの時間帯 (0..1 が 1 周)。
#[derive(Resource)]
pub struct DayCycle {
    pub t: f32,
}

impl Default for DayCycle {
    fn default() -> Self {
        Self { t: 0.12 }
    }
}

const DAY_LENGTH: f32 = 540.0;

pub struct RenderSetupPlugin;

impl Plugin for RenderSetupPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DirectionalLightShadowMap { size: 2048 })
            .insert_resource(ClearColor(Color::srgb(0.5, 0.7, 0.9)))
            .init_resource::<DayCycle>()
            .add_systems(Startup, spawn_sun)
            .add_systems(Update, (on_zone_entered, update_environment, animate_water).chain());
    }
}

fn spawn_sun(mut commands: Commands, controls: Res<crate::input::Controls>) {
    commands.spawn((
        Sun,
        DirectionalLight {
            illuminance: 11_000.0,
            // スマホでは軽くするため影を使わない
            shadow_maps_enabled: !controls.touch_ui,
            color: Color::srgb(1.0, 0.95, 0.85),
            ..default()
        },
        CascadeShadowConfigBuilder {
            num_cascades: 1,
            minimum_distance: 0.1,
            maximum_distance: 70.0,
            first_cascade_far_bound: 70.0,
            overlap_proportion: 0.2,
        }
        .build(),
        Transform::from_xyz(0.0, 50.0, 0.0).looking_at(Vec3::new(-0.4, 0.0, 0.3), Vec3::Y),
    ));
}

fn sky_mesh() -> Mesh {
    let mut lp = LpMesh::default();
    let rings = 14;
    let seg = 32;
    let r = 600.0;
    let col = |v: f32| {
        // v: -0.2 (地平線の下) .. 1 (天頂)
        let horizon = Color::srgb(0.86, 0.9, 0.96);
        let mid = Color::srgb(0.52, 0.72, 0.95);
        let top = Color::srgb(0.22, 0.42, 0.82);
        let below = Color::srgb(0.62, 0.68, 0.75);
        if v < 0.0 {
            below.mix(&horizon, (v + 0.2) / 0.2)
        } else if v < 0.3 {
            horizon.mix(&mid, v / 0.3)
        } else {
            mid.mix(&top, (v - 0.3) / 0.7)
        }
    };
    for i in 0..rings {
        let v0 = -0.2 + i as f32 / rings as f32 * 1.2;
        let v1 = -0.2 + (i + 1) as f32 / rings as f32 * 1.2;
        let e0 = v0.clamp(-1.0, 1.0).asin();
        let e1 = v1.clamp(-1.0, 1.0).asin();
        for s in 0..seg {
            let a0 = s as f32 / seg as f32 * TAU;
            let a1 = (s + 1) as f32 / seg as f32 * TAU;
            let p = |a: f32, e: f32| Vec3::new(a.cos() * e.cos(), e.sin(), a.sin() * e.cos()) * r;
            let (c0, c1) = (col(v0), col(v1));
            let (pa, pb, pc, pd) = (p(a0, e0), p(a1, e0), p(a1, e1), p(a0, e1));
            for (tri, cs) in [([pa, pb, pc], [c0, c0, c1]), ([pa, pc, pd], [c0, c1, c1])] {
                for k in 0..3 {
                    lp.pos.push(tri[k].to_array());
                    lp.nrm.push((-tri[k].normalize()).to_array());
                    lp.col.push(cs[k].to_linear().to_f32_array());
                }
            }
        }
    }
    lp.build()
}

fn spawn_sky(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &mut Assets<StandardMaterial>, ga: &GameAssets) {
    let sky_mat = mats.add(StandardMaterial {
        base_color: Color::WHITE,
        unlit: true,
        fog_enabled: false,
        cull_mode: None,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(sky_mesh())),
        MeshMaterial3d(sky_mat),
        Transform::IDENTITY,
        SkyDome,
        NotShadowCaster,
        ZoneEntity,
    ));
    let sun_mat = mats.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.95, 0.8),
        emissive: LinearRgba::rgb(60.0, 50.0, 30.0),
        unlit: true,
        fog_enabled: false,
        ..default()
    });
    commands.spawn((
        Mesh3d(ga.sphere.clone()),
        MeshMaterial3d(sun_mat),
        Transform::from_scale(Vec3::splat(14.0)),
        SunDisc,
        NotShadowCaster,
        ZoneEntity,
    ));
    // 雲
    let cloud_mat = mats.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.92),
        emissive: LinearRgba::rgb(0.5, 0.5, 0.55),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 1.0,
        fog_enabled: false,
        ..default()
    });
    let root = commands
        .spawn((Transform::IDENTITY, Visibility::default(), Clouds, ZoneEntity))
        .id();
    let mut rng = Rng::new(808);
    for _ in 0..22 {
        let a = rng.range(0.0, TAU);
        let d = rng.range(180.0, 420.0);
        let c = Vec3::new(a.cos() * d, rng.range(90.0, 150.0), a.sin() * d);
        let n = 3 + rng.pick(4);
        let size = rng.range(14.0, 26.0);
        for _ in 0..n {
            let off = Vec3::new(rng.range(-1.6, 1.6), rng.range(-0.2, 0.4), rng.range(-0.8, 0.8)) * size;
            let s = size * rng.range(0.6, 1.1);
            let e = commands
                .spawn((
                    Mesh3d(ga.blobs[rng.pick(ga.blobs.len())].clone()),
                    MeshMaterial3d(cloud_mat.clone()),
                    Transform::from_translation(c + off).with_scale(Vec3::new(s * 1.4, s * 0.55, s)),
                    NotShadowCaster,
                ))
                .id();
            commands.entity(root).add_child(e);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn on_zone_entered(
    mut commands: Commands,
    mut msgs: MessageReader<ZoneEntered>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    ga: Res<GameAssets>,
) {
    for m in msgs.read() {
        if m.0 == ZoneId::Field {
            spawn_sky(&mut commands, &mut meshes, &mut mats, &ga);
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_environment(
    time: Res<Time>,
    zone: Res<CurrentZone>,
    mut day: ResMut<DayCycle>,
    mut sun: Query<(&mut DirectionalLight, &mut Transform), (With<Sun>, Without<Player>, Without<MainCamera>)>,
    mut disc: Query<&mut Transform, (With<SunDisc>, Without<Sun>, Without<Player>, Without<MainCamera>)>,
    mut clouds: Query<&mut Transform, (With<Clouds>, Without<Sun>, Without<SunDisc>, Without<Player>, Without<MainCamera>)>,
    mut sky: Query<&mut Transform, (With<SkyDome>, Without<Clouds>, Without<Sun>, Without<SunDisc>, Without<Player>, Without<MainCamera>)>,
    player: Query<&Transform, (With<Player>, Without<MainCamera>)>,
    mut cam: Query<(&mut DistanceFog, &Transform), With<MainCamera>>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut clear: ResMut<ClearColor>,
) {
    let dt = time.delta_secs();
    let Ok((mut light, mut lt)) = sun.single_mut() else { return };
    let Ok((mut fog, cam_t)) = cam.single_mut() else { return };
    let ppos = player.single().map(|t| t.translation).unwrap_or(Vec3::ZERO);

    match zone.0 {
        ZoneId::Field => {
            day.t = (day.t + dt / DAY_LENGTH) % 1.0;
            // 太陽は低め〜高めを往復しながら一周する (夜にはならない)
            let az = day.t * TAU;
            let elev = 0.62 + 0.4 * (day.t * TAU * 2.0).sin();
            let dir = Vec3::new(az.cos() * elev.cos(), elev.sin(), az.sin() * elev.cos());
            let golden = (1.0 - (elev - 0.22) / 0.8).clamp(0.0, 1.0);
            let warm = Color::srgb(1.0, 0.72, 0.45);
            let noon = Color::srgb(1.0, 0.97, 0.9);
            light.color = noon.mix(&warm, golden * golden);
            light.illuminance = 7_000.0 + 7_000.0 * (1.0 - golden);
            light.shadow_maps_enabled = true;
            *lt = Transform::from_translation(ppos + dir * 60.0).looking_at(ppos, Vec3::Y);
            for mut d in &mut disc {
                d.translation = cam_t.translation + dir * 500.0;
            }
            for mut s in &mut sky {
                s.translation = cam_t.translation;
            }
            for mut c in &mut clouds {
                c.rotate_y(dt * 0.004);
                c.translation = Vec3::new(cam_t.translation.x, 0.0, cam_t.translation.z) * 0.9;
            }
            let fog_col = Color::srgb(0.72, 0.8, 0.9).mix(&Color::srgb(0.95, 0.78, 0.62), golden * 0.6);
            *fog = DistanceFog {
                color: fog_col,
                directional_light_color: light.color.with_alpha(0.55),
                directional_light_exponent: 24.0,
                falloff: FogFalloff::from_visibility_colors(
                    300.0,
                    Color::srgb(0.35, 0.5, 0.66),
                    Color::srgb(0.8, 0.84, 0.95),
                ),
            };
            ambient.color = Color::srgb(0.7, 0.8, 1.0).mix(&Color::srgb(1.0, 0.8, 0.7), golden * 0.5);
            ambient.brightness = 900.0;
            clear.0 = fog_col;
        }
        ZoneId::Dungeon => {
            light.illuminance = 0.0;
            light.shadow_maps_enabled = false;
            *fog = DistanceFog {
                color: Color::srgb(0.02, 0.015, 0.035),
                directional_light_color: Color::NONE,
                directional_light_exponent: 8.0,
                falloff: FogFalloff::Linear { start: 10.0, end: 55.0 },
            };
            ambient.color = Color::srgb(0.45, 0.4, 0.8);
            ambient.brightness = 220.0;
            clear.0 = Color::srgb(0.02, 0.015, 0.035);
        }
    }
}

fn animate_water(
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    q: Query<(&Mesh3d, &Water)>,
) {
    let t = time.elapsed_secs();
    for (m, w) in &q {
        let Some(mut mesh) = meshes.get_mut(&m.0) else { continue };
        let mut pos = Vec::with_capacity(w.base.len());
        for p in &w.base {
            let y = (p[0] * 0.35 + t * 1.3).sin() * 0.06
                + (p[2] * 0.5 - t * 1.1).sin() * 0.05
                + ((p[0] + p[2]) * 0.9 + t * 2.3).sin() * 0.025;
            pos.push([p[0], y, p[2]]);
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
        mesh.compute_smooth_normals();
    }
}
