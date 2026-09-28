//! ダメージ処理・ヒットストップ・撃破報酬・ドロップ品。

use bevy::{light::NotShadowCaster, prelude::*};

use crate::{
    assets::GameAssets,
    audio::{PlaySfx, SfxKind},
    camera::CamRig,
    enemy::{AiState, Enemy, EnemyKilled, EnemyKind},
    player::{PAction, Player, PlayerCtl},
    stats::{HUNT_TARGET, HuntQuest, PlayerStats, Progress},
    ui::{Banner, DamageNumber, HurtFlash},
    util::{GameRng, damp},
    vfx::{Burst, burst},
    world_running,
    zone::ZoneEntity,
};

#[derive(Message, Clone, Copy)]
pub struct Damage {
    pub target: Entity,
    pub amount: f32,
    pub crit: bool,
    pub from: Vec3,
    pub knock: f32,
    pub heavy: bool,
}

#[derive(Resource, Default)]
pub struct HitStop {
    pub remaining: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PickupKind {
    Potion,
    Gold(u32),
}

#[derive(Component)]
pub struct Pickup {
    pub kind: PickupKind,
    pub t: f32,
    pub base: Vec3,
}

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Damage>()
            .init_resource::<HitStop>()
            .add_systems(
                Update,
                (apply_damage, on_killed, pickups).chain().run_if(world_running),
            )
            .add_systems(Last, hitstop);
    }
}

fn hitstop(real: Res<Time<Real>>, mut hs: ResMut<HitStop>, mut vt: ResMut<Time<Virtual>>) {
    if hs.remaining > 0.0 {
        hs.remaining -= real.delta_secs();
        vt.set_relative_speed(0.04);
    } else {
        vt.set_relative_speed(1.0);
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_damage(
    mut msgs: MessageReader<Damage>,
    mut commands: Commands,
    ga: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut stats: ResMut<PlayerStats>,
    mut player: Query<(Entity, &Transform, &mut PlayerCtl), With<Player>>,
    mut enemies: Query<(&Transform, &mut Enemy), Without<Player>>,
    mut sfx: MessageWriter<PlaySfx>,
    mut killed: MessageWriter<EnemyKilled>,
    mut cam: ResMut<CamRig>,
    mut hs: ResMut<HitStop>,
    mut flash: ResMut<HurtFlash>,
) {
    let Ok((pe, pt, mut ctl)) = player.single_mut() else {
        msgs.clear();
        return;
    };
    for d in msgs.read() {
        if d.target == pe {
            if ctl.action == PAction::Dead || stats.hp <= 0.0 {
                continue;
            }
            if ctl.iframes > 0.0 {
                // 回避成功
                continue;
            }
            let amount = ((d.amount - stats.def() * 0.6).max(1.0) * rng.0.range(0.92, 1.08)).round();
            stats.hp = (stats.hp - amount).max(0.0);
            let dir = pt.translation - d.from;
            let dir = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
            ctl.knock = dir * d.knock;
            ctl.iframes = 0.45;
            if ctl.action != PAction::Roll {
                ctl.action = PAction::Hurt;
                ctl.t = 0.0;
                ctl.step = 0;
            }
            sfx.write(PlaySfx(SfxKind::PlayerHurt));
            cam.shake(if d.heavy { 0.55 } else { 0.35 });
            hs.remaining = hs.remaining.max(0.05);
            flash.0 = 1.0;
            burst(&mut commands, &ga, &mut rng.0, Burst::Blood, pt.translation + Vec3::Y * 1.1);
            commands.spawn(DamageNumber::new(pt.translation + Vec3::Y * 2.0, format!("{amount}"), Color::srgb(1.0, 0.35, 0.3), 26.0));
            continue;
        }
        let Ok((et, mut en)) = enemies.get_mut(d.target) else { continue };
        if en.dead {
            continue;
        }
        let def = en.kind.def();
        let amount = (d.amount - en.def * 0.5).max(1.0).round();
        en.hp -= amount;
        en.flash = 1.0;
        en.last_hit = 0.0;
        let dir = et.translation - d.from;
        let dir = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
        let weight = match en.kind {
            EnemyKind::Golem => 0.25,
            EnemyKind::Boss => 0.12,
            _ => 1.0,
        };
        en.knock = dir * d.knock * weight;
        en.poise += amount * if d.heavy { 1.5 } else { 1.0 };
        let stagger = def.poise <= 0.0
            || en.poise >= def.poise
            || (en.kind == EnemyKind::Skeleton && en.state != AiState::Attack);
        if stagger && en.state != AiState::Dormant && en.state != AiState::Roar {
            if def.poise > 0.0 && en.poise >= def.poise {
                en.poise = 0.0;
            }
            en.state = AiState::Hurt;
            en.t = 0.0;
        } else if matches!(en.state, AiState::Idle | AiState::Wander) {
            en.state = AiState::Chase;
        }
        let hit_pos = et.translation + Vec3::Y * (def.bar_height * 0.45) - dir * en.radius * 0.6;
        burst(&mut commands, &ga, &mut rng.0, if d.crit { Burst::Crit } else { Burst::Hit }, hit_pos);
        sfx.write(PlaySfx(if d.crit { SfxKind::Crit } else { SfxKind::Hit }));
        hs.remaining = hs.remaining.max(if d.crit || d.heavy { 0.085 } else { 0.05 });
        cam.shake(if d.crit { 0.28 } else { 0.14 });
        commands.spawn(DamageNumber::new(
            et.translation + Vec3::Y * def.bar_height,
            format!("{amount}"),
            if d.crit { Color::srgb(1.0, 0.85, 0.2) } else { Color::WHITE },
            if d.crit { 34.0 } else { 26.0 },
        ));
        if en.hp <= 0.0 {
            en.hp = 0.0;
            en.dead = true;
            en.state = AiState::Dying;
            en.t = 0.0;
            en.vel = Vec3::ZERO;
            killed.write(EnemyKilled { kind: en.kind, pos: et.translation });
            sfx.write(PlaySfx(if en.kind == EnemyKind::Boss { SfxKind::Roar } else { SfxKind::EnemyDie }));
            hs.remaining = if en.kind == EnemyKind::Boss { 0.5 } else { 0.1 };
            if ctl.lock == Some(d.target) {
                ctl.lock = None;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn on_killed(
    mut msgs: MessageReader<EnemyKilled>,
    mut commands: Commands,
    ga: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut stats: ResMut<PlayerStats>,
    mut progress: ResMut<Progress>,
    player: Query<&Transform, With<Player>>,
    mut sfx: MessageWriter<PlaySfx>,
    mut banner: MessageWriter<Banner>,
) {
    for k in msgs.read() {
        let def = k.kind.def();
        stats.kills += 1;
        let ups = stats.add_exp(def.exp);
        commands.spawn(DamageNumber::new(k.pos + Vec3::Y * (def.bar_height + 0.6), format!("+{} EXP", def.exp), Color::srgb(0.6, 0.85, 1.0), 20.0));
        if ups > 0 {
            sfx.write(PlaySfx(SfxKind::LevelUp));
            banner.write(Banner::small(&format!("レベルアップ!  Lv {}", stats.level)));
            if let Ok(pt) = player.single() {
                burst(&mut commands, &ga, &mut rng.0, Burst::LevelUp, pt.translation);
            }
        }
        if k.kind == EnemyKind::Wolf && progress.hunt == HuntQuest::Active {
            progress.wolves += 1;
            if progress.wolves >= HUNT_TARGET {
                progress.hunt = HuntQuest::Done;
                banner.write(Banner::small("狼の討伐完了! 狩人に報告しよう"));
            }
        }
        // ドロップ
        let n = (def.gold as f32 / 5.0).ceil().max(1.0) as u32;
        for i in 0..n {
            let amount = def.gold / n + if i == 0 { def.gold % n } else { 0 };
            spawn_pickup(&mut commands, &ga, &mut rng.0, k.pos, PickupKind::Gold(amount.max(1)));
        }
        let potion_chance = match k.kind {
            EnemyKind::Slime => 0.08,
            EnemyKind::Wolf => 0.15,
            EnemyKind::Skeleton => 0.2,
            EnemyKind::Golem => 0.6,
            EnemyKind::Boss => 1.0,
        };
        if rng.0.chance(potion_chance) {
            spawn_pickup(&mut commands, &ga, &mut rng.0, k.pos, PickupKind::Potion);
        }
    }
}

pub fn spawn_pickup(commands: &mut Commands, ga: &GameAssets, rng: &mut crate::util::Rng, pos: Vec3, kind: PickupKind) {
    let off = rng.unit_vec2() * rng.range(0.3, 1.4);
    let base = pos + Vec3::new(off.x, 0.6, off.y);
    let (mesh, mat, scale) = match kind {
        PickupKind::Gold(_) => (ga.cylinder.clone(), ga.fx_gold.clone(), Vec3::new(0.3, 0.06, 0.3)),
        PickupKind::Potion => (ga.sphere.clone(), ga.fx_blood.clone(), Vec3::splat(0.22)),
    };
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_translation(base).with_scale(scale),
        Pickup { kind, t: rng.range(0.0, 6.0), base },
        NotShadowCaster,
        ZoneEntity,
    ));
}

fn pickups(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &mut Transform, &mut Pickup), Without<Player>>,
    player: Query<&Transform, With<Player>>,
    mut stats: ResMut<PlayerStats>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let dt = time.delta_secs();
    let Ok(pt) = player.single() else { return };
    let target = pt.translation + Vec3::Y * 0.9;
    for (e, mut tf, mut p) in &mut q {
        p.t += dt;
        let d = target.distance(tf.translation);
        if p.t > 0.6 && d < 4.5 {
            // 吸い寄せ
            let k = damp(6.0 + (4.5 - d) * 3.0, dt);
            tf.translation = tf.translation.lerp(target, k);
            p.base = tf.translation;
        } else {
            tf.translation = p.base + Vec3::Y * ((p.t * 3.0).sin() * 0.12);
        }
        tf.rotate_y(dt * 3.0);
        if p.t > 0.6 && d < 0.8 {
            match p.kind {
                PickupKind::Gold(n) => {
                    stats.gold += n;
                    sfx.write(PlaySfx(SfxKind::Coin));
                }
                PickupKind::Potion => {
                    stats.potions = (stats.potions + 1).min(9);
                    sfx.write(PlaySfx(SfxKind::Pickup));
                    commands.spawn(DamageNumber::new(target + Vec3::Y, "回復薬 +1".into(), Color::srgb(0.6, 1.0, 0.6), 20.0));
                }
            }
            commands.entity(e).despawn();
        }
    }
}
