//! プレイヤーキャラクター。

use std::f32::consts::PI;

use bevy::prelude::*;

use crate::{
    GameState,
    assets::GameAssets,
    audio::{PlaySfx, SfxKind},
    camera::MainCamera,
    combat::Damage,
    enemy::Enemy,
    gameplay_active,
    input::Controls,
    rig::{self, BladeEdge, BladeMesh, Look, Pose, PoseAction, SwordBase, SwordTip, WeaponLook},
    stats::PlayerStats,
    util::{GameRng, damp, lerp_angle},
    vfx::{Burst, burst, spawn_trail},
    world_running,
    zone::{Colliders, CurrentZone, blocked, ground_height},
};

#[derive(Component)]
pub struct Player;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PAction {
    #[default]
    Idle,
    Attack,
    Roll,
    Hurt,
    Drink,
    Dead,
}

#[derive(Component, Default)]
pub struct PlayerCtl {
    pub vel: Vec3,
    pub grounded: bool,
    pub facing: f32,
    pub action: PAction,
    pub t: f32,
    pub step: u8,
    pub buffered: bool,
    pub hit_list: Vec<Entity>,
    pub stamina_cd: f32,
    pub iframes: f32,
    pub lock: Option<Entity>,
    pub knock: Vec3,
    pub roll_dir: Vec3,
    pub footstep: f32,
    pub prev_tip: Option<Vec3>,
    pub healed: bool,
    pub air_time: f32,
}

pub struct AttackStep {
    pub dur: f32,
    pub start: f32,
    pub end: f32,
    pub mult: f32,
    pub reach: f32,
    pub arc: f32,
    pub lunge: f32,
    pub pose: PoseAction,
}

pub const STEPS: [AttackStep; 3] = [
    AttackStep { dur: 0.46, start: 0.13, end: 0.26, mult: 1.0, reach: 2.7, arc: 1.05, lunge: 3.0, pose: PoseAction::Slash1 },
    AttackStep { dur: 0.48, start: 0.13, end: 0.27, mult: 1.15, reach: 2.7, arc: 1.05, lunge: 3.2, pose: PoseAction::Slash2 },
    AttackStep { dur: 0.72, start: 0.27, end: 0.40, mult: 1.9, reach: 3.1, arc: 0.8, lunge: 5.0, pose: PoseAction::Slam },
];

const RUN_SPEED: f32 = 7.2;
const GRAVITY: f32 = 26.0;
const RADIUS: f32 = 0.4;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            .add_systems(
                Update,
                (
                    (lock_on, player_control).chain().run_if(gameplay_active),
                    player_physics.run_if(world_running),
                    (sword_trail, player_pose).run_if(world_running),
                    update_blade_look,
                )
                    .chain(),
            )
            .add_systems(Update, rig::animate_limbs.after(player_pose))
            .add_systems(Update, death_check.run_if(in_state(GameState::Playing)));
    }
}

pub fn weapon_look(ga: &GameAssets, weapon: usize) -> WeaponLook {
    WeaponLook {
        blade: ga.steel.clone(),
        edge: if weapon == 2 { ga.blade_glow.clone() } else { ga.steel.clone() },
        hilt: ga.leather.clone(),
        length: 0.95,
        width: 1.0,
    }
}

fn spawn_player(mut commands: Commands, ga: Res<GameAssets>) {
    let e = commands
        .spawn((
            Player,
            PlayerCtl::default(),
            Pose::default(),
            Transform::from_translation(crate::field::VILLAGE_SPAWN),
            Visibility::default(),
        ))
        .id();
    let mut look = Look::basic(&ga);
    look.cape = Some(ga.cape.clone());
    look.weapon = Some(weapon_look(&ga, 0));
    look.shield = Some((ga.tunic.clone(), ga.steel.clone()));
    look.pauldrons = Some(ga.steel.clone());
    look.boots = ga.leather.clone();
    look.legs = ga.dark.clone();
    rig::build_humanoid(&mut commands, &ga, e, &look);
}

/// 装備に合わせて刃の見た目を変える。
fn update_blade_look(
    stats: Res<PlayerStats>,
    ga: Res<GameAssets>,
    player: Query<Entity, With<Player>>,
    mut edges: Query<(&BladeEdge, &mut MeshMaterial3d<StandardMaterial>), Without<BladeMesh>>,
    mut blades: Query<(&BladeMesh, &mut Transform)>,
    mut last: Local<Option<usize>>,
) {
    let Ok(pe) = player.single() else { return };
    if *last == Some(stats.weapon) {
        return;
    }
    *last = Some(stats.weapon);
    for (b, mut m) in &mut edges {
        if b.0 == pe {
            m.0 = if stats.weapon == 2 { ga.blade_glow.clone() } else { ga.steel.clone() };
        }
    }
    for (b, mut t) in &mut blades {
        if b.0 == pe {
            t.scale.x = 0.075 * [1.0, 1.25, 1.4][stats.weapon.min(2)];
        }
    }
}

fn nearest_target(pos: Vec3, facing: Vec3, enemies: &Query<(Entity, &Transform, &Enemy), Without<Player>>) -> Option<Entity> {
    let mut best = None;
    let mut best_score = f32::MAX;
    for (e, t, en) in enemies.iter() {
        if en.dead {
            continue;
        }
        let d = t.translation - pos;
        let dist = d.length();
        if dist > 24.0 {
            continue;
        }
        let dir = d.normalize_or_zero();
        let score = dist * (1.6 - dir.dot(facing));
        if score < best_score {
            best_score = score;
            best = Some(e);
        }
    }
    best
}

fn lock_on(
    controls: Res<Controls>,
    mut q: Query<(&Transform, &mut PlayerCtl), With<Player>>,
    enemies: Query<(Entity, &Transform, &Enemy), Without<Player>>,
    cam: Query<&Transform, (With<MainCamera>, Without<Player>, Without<Enemy>)>,
) {
    let Ok((tf, mut ctl)) = q.single_mut() else { return };
    if let Some(t) = ctl.lock {
        let valid = enemies
            .get(t)
            .map(|(_, et, en)| !en.dead && et.translation.distance(tf.translation) < 30.0)
            .unwrap_or(false);
        if !valid {
            ctl.lock = None;
        }
    }
    if controls.lock_on {
        if ctl.lock.is_some() {
            ctl.lock = None;
        } else {
            let fwd = cam
                .single()
                .map(|c| {
                    let f = c.forward().as_vec3();
                    Vec3::new(f.x, 0.0, f.z).normalize_or_zero()
                })
                .unwrap_or(Vec3::NEG_Z);
            ctl.lock = nearest_target(tf.translation, fwd, &enemies);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn player_control(
    time: Res<Time>,
    controls: Res<Controls>,
    mut stats: ResMut<PlayerStats>,
    mut q: Query<(&Transform, &mut PlayerCtl), With<Player>>,
    cam: Query<&Transform, (With<MainCamera>, Without<Player>)>,
    enemies: Query<(Entity, &Transform, &Enemy), Without<Player>>,
    mut sfx: MessageWriter<PlaySfx>,
    mut dmg: MessageWriter<Damage>,
    mut rng: ResMut<GameRng>,
    mut commands: Commands,
    ga: Res<GameAssets>,
) {
    let dt = time.delta_secs();
    let Ok((tf, mut ctl)) = q.single_mut() else { return };
    let Ok(cam_tf) = cam.single() else { return };
    let max_st = stats.max_stamina();

    // スタミナ回復
    ctl.stamina_cd -= dt;
    if ctl.stamina_cd <= 0.0 && ctl.action != PAction::Attack && ctl.action != PAction::Roll {
        stats.stamina = (stats.stamina + 38.0 * dt).min(max_st);
    }
    ctl.iframes -= dt;

    let cf = cam_tf.forward().as_vec3();
    let fwd = Vec3::new(cf.x, 0.0, cf.z).normalize_or_zero();
    let right = Vec3::new(-fwd.z, 0.0, fwd.x);
    let wish = (fwd * controls.movement.y + right * controls.movement.x).clamp_length_max(1.0);

    let lock_dir = ctl.lock.and_then(|e| enemies.get(e).ok()).map(|(_, et, _)| {
        let d = et.translation - tf.translation;
        Vec3::new(d.x, 0.0, d.z).normalize_or_zero()
    });

    ctl.t += dt;
    let action = ctl.action;
    match action {
        PAction::Dead => {
            ctl.vel.x = 0.0;
            ctl.vel.z = 0.0;
            return;
        }
        PAction::Hurt => {
            if ctl.t > 0.38 {
                ctl.action = PAction::Idle;
            }
        }
        PAction::Drink => {
            if ctl.t > 0.5 && !ctl.healed {
                ctl.healed = true;
                let heal = stats.max_hp() * 0.45;
                stats.hp = (stats.hp + heal).min(stats.max_hp());
                sfx.write(PlaySfx(SfxKind::Heal));
                burst(&mut commands, &ga, &mut rng.0, Burst::Heal, tf.translation + Vec3::Y * 1.0);
            }
            if ctl.t > 0.95 {
                ctl.action = PAction::Idle;
            }
        }
        PAction::Roll => {
            let k = 1.0 - (ctl.t / 0.52);
            let dir = ctl.roll_dir;
            ctl.vel.x = dir.x * (5.0 + 8.0 * k.max(0.0));
            ctl.vel.z = dir.z * (5.0 + 8.0 * k.max(0.0));
            if ctl.t > 0.52 {
                ctl.action = PAction::Idle;
            }
        }
        PAction::Attack => {
            let step = &STEPS[ctl.step as usize];
            if controls.attack && ctl.t > 0.08 {
                ctl.buffered = true;
            }
            // 踏み込み
            let f = Vec3::new(-ctl.facing.sin(), 0.0, -ctl.facing.cos());
            let lunge = if ctl.t < step.start + 0.05 { step.lunge } else { 0.0 };
            ctl.vel.x = f.x * lunge;
            ctl.vel.z = f.z * lunge;
            // 当たり判定
            // 低フレームレートでも判定区間を飛び越えないよう、区間を「通過したか」で判定する
            if ctl.t >= step.start && ctl.t - dt <= step.end {
                let atk = stats.atk();
                for (e, et, en) in enemies.iter() {
                    if en.dead || ctl.hit_list.contains(&e) {
                        continue;
                    }
                    let d = et.translation - tf.translation;
                    let flat = Vec3::new(d.x, 0.0, d.z);
                    let dist = flat.length() - en.radius;
                    if dist > step.reach || d.y.abs() > 2.5 + en.radius {
                        continue;
                    }
                    let ang = flat.normalize_or_zero().dot(f).acos();
                    if ang > step.arc && dist > 0.6 {
                        continue;
                    }
                    ctl.hit_list.push(e);
                    let crit = rng.0.chance(0.12);
                    let amount = atk * step.mult * rng.0.range(0.9, 1.1) * if crit { 1.8 } else { 1.0 };
                    dmg.write(Damage {
                        target: e,
                        amount,
                        crit,
                        from: tf.translation,
                        knock: if ctl.step == 2 { 7.0 } else { 3.5 },
                        heavy: ctl.step == 2,
                    });
                }
            }
            if ctl.t > step.end + 0.02 && ctl.buffered && ctl.step < 2 && stats.stamina > 0.0 {
                ctl.step += 1;
                start_attack(&mut ctl, &mut stats, wish, lock_dir, &mut sfx);
            } else if ctl.t > step.dur {
                ctl.action = PAction::Idle;
                ctl.step = 0;
            } else if controls.roll && ctl.t > step.end && stats.stamina >= 10.0 {
                start_roll(&mut ctl, &mut stats, wish, &mut sfx);
            }
        }
        PAction::Idle => {
            let speed = RUN_SPEED;
            let target = wish * speed;
            let k = damp(if ctl.grounded { 14.0 } else { 3.0 }, dt);
            ctl.vel.x += (target.x - ctl.vel.x) * k;
            ctl.vel.z += (target.z - ctl.vel.z) * k;

            let face_dir = if let Some(ld) = lock_dir { Some(ld) } else if wish.length() > 0.1 { Some(wish) } else { None };
            if let Some(d) = face_dir {
                let yaw = (-d.x).atan2(-d.z);
                ctl.facing = lerp_angle(ctl.facing, yaw, damp(14.0, dt));
            }

            if controls.jump && ctl.grounded {
                ctl.vel.y = 9.0;
                ctl.grounded = false;
                sfx.write(PlaySfx(SfxKind::Jump));
            } else if controls.attack && stats.stamina > 0.0 {
                ctl.step = 0;
                start_attack(&mut ctl, &mut stats, wish, lock_dir, &mut sfx);
            } else if controls.roll && stats.stamina >= 10.0 && ctl.grounded {
                start_roll(&mut ctl, &mut stats, wish, &mut sfx);
            } else if controls.potion && stats.potions > 0 && stats.hp < stats.max_hp() && ctl.grounded {
                stats.potions -= 1;
                ctl.action = PAction::Drink;
                ctl.t = 0.0;
                ctl.healed = false;
                sfx.write(PlaySfx(SfxKind::Potion));
            }

            // 足元の砂ぼこり
            let hs = Vec2::new(ctl.vel.x, ctl.vel.z).length();
            if ctl.grounded && hs > 4.0 {
                ctl.footstep -= dt;
                if ctl.footstep <= 0.0 {
                    ctl.footstep = 0.3;
                    burst(&mut commands, &ga, &mut rng.0, Burst::Step, tf.translation + Vec3::Y * 0.1);
                    sfx.write(PlaySfx(SfxKind::Step));
                }
            }
        }
    }
    if ctl.action == PAction::Drink {
        ctl.vel.x *= 0.85;
        ctl.vel.z *= 0.85;
    }
}

fn start_attack(ctl: &mut PlayerCtl, stats: &mut PlayerStats, wish: Vec3, lock_dir: Option<Vec3>, sfx: &mut MessageWriter<PlaySfx>) {
    ctl.action = PAction::Attack;
    ctl.t = 0.0;
    ctl.buffered = false;
    ctl.hit_list.clear();
    ctl.prev_tip = None;
    stats.stamina -= if ctl.step == 2 { 16.0 } else { 11.0 };
    ctl.stamina_cd = 0.7;
    let dir = lock_dir.or(if wish.length() > 0.2 { Some(wish) } else { None });
    if let Some(d) = dir {
        ctl.facing = (-d.x).atan2(-d.z);
    }
    sfx.write(PlaySfx(if ctl.step == 2 { SfxKind::SwingHeavy } else { SfxKind::Swing }));
}

fn start_roll(ctl: &mut PlayerCtl, stats: &mut PlayerStats, wish: Vec3, sfx: &mut MessageWriter<PlaySfx>) {
    ctl.action = PAction::Roll;
    ctl.t = 0.0;
    ctl.iframes = 0.38;
    stats.stamina -= 22.0;
    ctl.stamina_cd = 0.7;
    ctl.step = 0;
    let dir = if wish.length() > 0.2 {
        wish.normalize()
    } else {
        Vec3::new(-ctl.facing.sin(), 0.0, -ctl.facing.cos())
    };
    ctl.roll_dir = dir;
    ctl.facing = (-dir.x).atan2(-dir.z);
    sfx.write(PlaySfx(SfxKind::Roll));
}

#[allow(clippy::too_many_arguments)]
fn player_physics(
    time: Res<Time>,
    zone: Res<CurrentZone>,
    colliders: Res<Colliders>,
    mut q: Query<(&mut Transform, &mut PlayerCtl), With<Player>>,
    mut commands: Commands,
    ga: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let dt = time.delta_secs().min(0.05);
    let Ok((mut tf, mut ctl)) = q.single_mut() else { return };
    let z = zone.0;

    // ノックバック
    let knock = ctl.knock;
    ctl.knock = knock * (1.0 - damp(8.0, dt));

    let old = tf.translation.xz();
    let mut p = old + (Vec2::new(ctl.vel.x, ctl.vel.z) + knock.xz()) * dt;
    p = colliders.resolve(p, RADIUS);
    if blocked(z, p) {
        // 軸ごとに試してスライドさせる
        let px = Vec2::new(p.x, old.y);
        let pz = Vec2::new(old.x, p.y);
        p = if !blocked(z, px) {
            px
        } else if !blocked(z, pz) {
            pz
        } else {
            old
        };
    }
    tf.translation.x = p.x;
    tf.translation.z = p.y;

    let g = ground_height(z, p);
    ctl.vel.y -= GRAVITY * dt;
    tf.translation.y += ctl.vel.y * dt;
    let was_air = !ctl.grounded;
    if tf.translation.y <= g || (ctl.grounded && tf.translation.y - g < 0.45 && ctl.vel.y <= 0.0) {
        if was_air && ctl.air_time > 0.35 {
            burst(&mut commands, &ga, &mut rng.0, Burst::Land, Vec3::new(p.x, g + 0.1, p.y));
            sfx.write(PlaySfx(SfxKind::Land));
        }
        tf.translation.y = g;
        ctl.vel.y = 0.0;
        ctl.grounded = true;
        ctl.air_time = 0.0;
    } else {
        ctl.grounded = false;
        ctl.air_time += dt;
    }
    tf.rotation = Quat::from_rotation_y(ctl.facing);
}

fn player_pose(
    time: Res<Time>,
    stats: Res<PlayerStats>,
    mut q: Query<(&PlayerCtl, &mut Pose), With<Player>>,
) {
    let dt = time.delta_secs();
    let Ok((ctl, mut pose)) = q.single_mut() else { return };
    let hs = Vec2::new(ctl.vel.x, ctl.vel.z).length();
    pose.clock += dt;
    pose.moving += ((hs / RUN_SPEED).min(1.0) - pose.moving) * damp(10.0, dt);
    if ctl.action == PAction::Attack || ctl.action == PAction::Roll {
        pose.moving *= 0.5;
    }
    pose.phase += hs * dt * 1.35;
    pose.air = !ctl.grounded && ctl.action != PAction::Roll;
    pose.hurt = if ctl.action == PAction::Hurt { (1.0 - ctl.t / 0.38).max(0.0) } else { 0.0 };
    let (action, t) = match ctl.action {
        PAction::Attack => {
            let s = &STEPS[ctl.step as usize];
            (s.pose, (ctl.t / s.dur).min(1.0))
        }
        PAction::Roll => (PoseAction::Roll, (ctl.t / 0.52).min(1.0)),
        PAction::Drink => (PoseAction::Drink, (ctl.t / 0.95).min(1.0)),
        _ => (PoseAction::None, 0.0),
    };
    pose.action = action;
    pose.t = t;
    pose.dead = if ctl.action == PAction::Dead { (ctl.t * 2.0).min(1.0) } else { 0.0 };
    let _ = stats;
}

fn sword_trail(
    mut q: Query<(Entity, &mut PlayerCtl), With<Player>>,
    tips: Query<(&GlobalTransform, &SwordTip)>,
    bases: Query<(&GlobalTransform, &SwordBase)>,
    mut commands: Commands,
    ga: Res<GameAssets>,
) {
    let Ok((pe, mut ctl)) = q.single_mut() else { return };
    if ctl.action != PAction::Attack {
        ctl.prev_tip = None;
        return;
    }
    let step = &STEPS[ctl.step as usize];
    if ctl.t < step.start - 0.05 || ctl.t > step.end + 0.04 {
        return;
    }
    let tip = tips.iter().find(|(_, o)| o.0 == pe).map(|(g, _)| g);
    let base = bases.iter().find(|(_, o)| o.0 == pe).map(|(g, _)| g);
    let (Some(tip), Some(base)) = (tip, base) else { return };
    let tip = tip.translation();
    let base = base.translation();
    if let Some(prev) = ctl.prev_tip {
        spawn_trail(&mut commands, &ga, base, tip, prev);
    }
    ctl.prev_tip = Some(tip);
}

fn death_check(
    stats: Res<PlayerStats>,
    mut q: Query<&mut PlayerCtl, With<Player>>,
    mut next: ResMut<NextState<GameState>>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let Ok(mut ctl) = q.single_mut() else { return };
    if stats.hp <= 0.0 && ctl.action != PAction::Dead {
        ctl.action = PAction::Dead;
        ctl.t = 0.0;
        ctl.lock = None;
        sfx.write(PlaySfx(SfxKind::PlayerDie));
    }
    if ctl.action == PAction::Dead
        && ctl.t > 2.6 {
            next.set(GameState::GameOver);
        }
}

/// 復活 (ゲームオーバー後・ロード時)。
pub fn revive(ctl: &mut PlayerCtl) {
    ctl.action = PAction::Idle;
    ctl.t = 0.0;
    ctl.vel = Vec3::ZERO;
    ctl.knock = Vec3::ZERO;
    ctl.lock = None;
    ctl.facing = PI;
}
