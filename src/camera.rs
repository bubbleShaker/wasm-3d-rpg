//! 三人称オービットカメラ。

use bevy::{
    anti_alias::fxaa::Fxaa,
    camera::Hdr,
    core_pipeline::tonemapping::Tonemapping,
    light::ShadowFilteringMethod,
    post_process::bloom::Bloom,
    prelude::*,
    render::view::Msaa,
};

use crate::{
    GameState,
    enemy::Enemy,
    input::Controls,
    player::{Player, PlayerCtl},
    util::{damp, lerp_angle, value_noise},
    zone::{Colliders, CurrentZone, ZoneId, ground_height},
};

const MIN_VIEW_DIST: f32 = 3.2;

#[derive(Component)]
pub struct MainCamera;

#[derive(Resource)]
pub struct CamRig {
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub focus: Vec3,
    pub trauma: f32,
    pub cur_dist: f32,
    pub lift: f32,
    pub time: f32,
}

impl Default for CamRig {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.38,
            dist: 7.5,
            focus: Vec3::ZERO,
            trauma: 0.0,
            cur_dist: 7.5,
            lift: 0.0,
            time: 0.0,
        }
    }
}

impl CamRig {
    pub fn shake(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }
}

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CamRig>()
            .add_systems(Startup, spawn_camera)
            .add_systems(
                PostUpdate,
                follow_camera.before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

fn spawn_camera(mut commands: Commands, controls: Res<Controls>) {
    let mut camera = commands.spawn((
        Camera3d::default(),
        MainCamera,
        Hdr,
        Tonemapping::TonyMcMapface,
        Msaa::Off,
        Fxaa::default(),
        // スマホは影のぼかしを軽いものにする
        if controls.touch_ui { ShadowFilteringMethod::Hardware2x2 } else { ShadowFilteringMethod::Gaussian },
        Projection::Perspective(PerspectiveProjection {
            fov: 55f32.to_radians(),
            far: 900.0,
            ..default()
        }),
        DistanceFog::default(),
        Transform::from_xyz(0.0, 12.0, 30.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    // スマホでは軽くするためブルームを使わない
    if !controls.touch_ui {
        camera.insert(Bloom {
            intensity: 0.18,
            ..Bloom::NATURAL
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn follow_camera(
    time: Res<Time<Real>>,
    vtime: Res<Time>,
    state: Res<State<GameState>>,
    controls: Res<Controls>,
    zone: Res<CurrentZone>,
    colliders: Res<Colliders>,
    mut rig: ResMut<CamRig>,
    player: Query<(&Transform, &PlayerCtl), With<Player>>,
    enemies: Query<&Transform, (With<Enemy>, Without<Player>, Without<MainCamera>)>,
    mut cam: Query<&mut Transform, (With<MainCamera>, Without<Player>, Without<Enemy>)>,
    block: Res<crate::UiBlock>,
) {
    let dt = time.delta_secs().min(0.05);
    rig.time += dt;
    let Ok(mut ct) = cam.single_mut() else { return };

    if matches!(state.get(), GameState::Title | GameState::Boot) {
        // タイトル画面: 村の上空をゆっくり旋回
        let a = rig.time * 0.05;
        let center = Vec3::new(0.0, 3.0, 0.0);
        let pos = center + Vec3::new(a.cos() * 34.0, 13.0 + (rig.time * 0.2).sin() * 2.0, a.sin() * 34.0);
        ct.translation = pos;
        ct.look_at(center + Vec3::new(0.0, 2.0, 0.0), Vec3::Y);
        return;
    }
    let Ok((pt, ctl)) = player.single() else { return };

    let playing = *state.get() == GameState::Playing && !block.any();
    if playing {
        rig.yaw -= controls.look.x;
        rig.pitch = (rig.pitch + controls.look.y).clamp(-0.35, 1.2);
        rig.dist = (rig.dist + controls.zoom * 0.8).clamp(3.5, 13.0);
    }

    // ロックオン中は敵の方向へ自動で向く
    if let Some(target) = ctl.lock.and_then(|e| enemies.get(e).ok()) {
        let d = target.translation - pt.translation;
        let want = d.x.atan2(d.z) + std::f32::consts::PI;
        rig.yaw = lerp_angle(rig.yaw, want, damp(5.0, dt));
        rig.pitch += (0.3 - rig.pitch) * damp(3.0, dt);
    }

    let target = pt.translation + Vec3::Y * 1.55;
    let k = damp(12.0, dt);
    rig.focus = if rig.focus == Vec3::ZERO || rig.focus.distance(target) > 20.0 {
        target
    } else {
        rig.focus.lerp(target, k)
    };

    let dir = Vec3::new(
        rig.yaw.sin() * rig.pitch.cos(),
        rig.pitch.sin(),
        rig.yaw.cos() * rig.pitch.cos(),
    );
    let mut want_dist = rig.dist;
    // 壁との衝突
    if zone.0 == ZoneId::Dungeon {
        let to = rig.focus + dir * want_dist;
        let f = colliders.ray_clear_fraction(rig.focus.xz(), to.xz());
        want_dist *= f;
    }
    rig.cur_dist = if want_dist < rig.cur_dist {
        want_dist
    } else {
        rig.cur_dist + (want_dist - rig.cur_dist) * damp(4.0, dt)
    };
    let mut pos = rig.focus + dir * rig.cur_dist;
    // 壁際で寄りすぎるときは、上へ逃げて見下ろす (体に埋まらない距離を保つ)
    let want_lift = (MIN_VIEW_DIST * MIN_VIEW_DIST - rig.cur_dist * rig.cur_dist).max(0.0).sqrt();
    rig.lift += (want_lift - rig.lift) * damp(8.0, dt);
    pos.y += rig.lift;
    let look = rig.focus - Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero() * rig.lift * 0.2;
    // 地形より下に潜らない
    let g = ground_height(zone.0, pos.xz()) + 0.5;
    if pos.y < g {
        pos.y = g;
    }
    if zone.0 == ZoneId::Dungeon {
        pos.y = pos.y.min(5.6);
    }

    // 画面揺れ
    rig.trauma = (rig.trauma - dt * 1.6).max(0.0);
    let s = rig.trauma * rig.trauma;
    let t = rig.time * 30.0;
    let shake = Vec3::new(
        value_noise(t, 0.0, 1) - 0.5,
        value_noise(t, 5.0, 2) - 0.5,
        value_noise(t, 9.0, 3) - 0.5,
    ) * s
        * 0.8;
    let _ = vtime;
    ct.translation = pos + shake;
    ct.look_at(look + shake * 0.5, Vec3::Y);
}
