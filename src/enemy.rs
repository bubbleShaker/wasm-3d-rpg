//! 敵キャラクターと AI。

use std::f32::consts::{PI, TAU};

use bevy::{light::NotShadowCaster, prelude::*};

use crate::{
    assets::GameAssets,
    audio::{Music, PlaySfx, SfxKind},
    camera::CamRig,
    combat::Damage,
    dungeon::{self, ARENA_CENTER, ARENA_RADIUS, GATE_CENTER},
    player::{PAction, Player, PlayerCtl},
    rig::{self, Look, Pose, PoseAction, RigStyle, WeaponLook},
    stats::{MainQuest, Progress},
    ui::Banner,
    util::{GameRng, damp, lerp_angle},
    vfx::{Burst, Timed, burst},
    world_running,
    zone::{Colliders, CurrentZone, Obb, ZoneEntity, ZoneId, blocked, ground_height},
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnemyKind {
    Slime,
    Wolf,
    Skeleton,
    Golem,
    Boss,
}

#[allow(dead_code)]
pub struct EnemyDef {
    pub name: &'static str,
    pub hp: f32,
    pub atk: f32,
    pub def: f32,
    pub speed: f32,
    pub aggro: f32,
    pub range: f32,
    pub radius: f32,
    pub exp: u32,
    pub gold: u32,
    pub poise: f32,
    pub bar_height: f32,
}

impl EnemyKind {
    pub fn def(self) -> EnemyDef {
        match self {
            EnemyKind::Slime => EnemyDef { name: "スライム", hp: 34.0, atk: 10.0, def: 0.0, speed: 2.6, aggro: 11.0, range: 3.6, radius: 0.55, exp: 7, gold: 3, poise: 0.0, bar_height: 1.4 },
            EnemyKind::Wolf => EnemyDef { name: "森の狼", hp: 62.0, atk: 14.0, def: 1.0, speed: 7.2, aggro: 16.0, range: 4.6, radius: 0.6, exp: 16, gold: 5, poise: 0.0, bar_height: 1.6 },
            EnemyKind::Skeleton => EnemyDef { name: "スケルトン", hp: 100.0, atk: 19.0, def: 3.0, speed: 3.7, aggro: 14.0, range: 2.5, radius: 0.5, exp: 28, gold: 10, poise: 12.0, bar_height: 2.3 },
            EnemyKind::Golem => EnemyDef { name: "古代ゴーレム", hp: 300.0, atk: 32.0, def: 8.0, speed: 2.4, aggro: 13.0, range: 3.4, radius: 1.2, exp: 85, gold: 30, poise: 70.0, bar_height: 3.8 },
            EnemyKind::Boss => EnemyDef { name: "黒騎士ヴァルガ", hp: 1700.0, atk: 30.0, def: 8.0, speed: 4.6, aggro: 80.0, range: 4.2, radius: 1.1, exp: 600, gold: 300, poise: 140.0, bar_height: 4.2 },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum AiState {
    #[default]
    Idle,
    Wander,
    Chase,
    Windup,
    Attack,
    Recover,
    Hurt,
    Dying,
    Dormant,
    Roar,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Move {
    #[default]
    Basic,
    Combo2,
    Leap,
    Charge,
    Orbs,
    Summon,
}

#[derive(Component)]
pub struct Enemy {
    pub kind: EnemyKind,
    pub hp: f32,
    pub max_hp: f32,
    pub atk: f32,
    pub def: f32,
    pub dead: bool,
    pub radius: f32,
    pub state: AiState,
    pub t: f32,
    pub home: Vec3,
    pub vel: Vec3,
    pub facing: f32,
    pub hit_done: bool,
    pub flash: f32,
    pub poise: f32,
    pub cooldown: f32,
    pub mv: Move,
    pub wander: Vec3,
    pub knock: Vec3,
    pub leap_from: Vec3,
    pub leap_to: Vec3,
    pub phase2: bool,
    pub summoned: bool,
    pub strafe: f32,
    pub combo: u8,
    pub last_hit: f32,
}

/// 敵の出現地点。倒されると一定時間後に復活する。
#[derive(Component)]
pub struct SpawnPoint {
    pub kind: EnemyKind,
    pub pos: Vec3,
    pub alive: Option<Entity>,
    pub timer: f32,
    pub respawn: bool,
    pub used: bool,
}

impl SpawnPoint {
    pub fn new(kind: EnemyKind, pos: Vec3) -> Self {
        Self { kind, pos, alive: None, timer: 0.0, respawn: true, used: false }
    }
    pub fn no_respawn(mut self) -> Self {
        self.respawn = false;
        self
    }
}

#[derive(Component)]
pub struct BossArena {
    pub active: bool,
}

#[derive(Component)]
pub struct BossGate;

#[derive(Component)]
pub struct Boss;

#[derive(Component)]
pub struct Orb {
    pub vel: Vec3,
    pub life: f32,
    pub dmg: f32,
}

/// 敵が倒れたときの通知。
#[derive(Message, Clone, Copy)]
pub struct EnemyKilled {
    pub kind: EnemyKind,
    pub pos: Vec3,
}

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<EnemyKilled>().add_systems(
            Update,
            (
                spawn_points,
                enemy_ai,
                enemy_physics,
                enemy_pose,
                orbs,
                boss_arena,
                enemy_dying,
            )
                .chain()
                .run_if(world_running)
                .before(rig::animate_limbs),
        );
    }
}

pub fn spawn_enemy(commands: &mut Commands, ga: &GameAssets, kind: EnemyKind, pos: Vec3, mult: f32) -> Entity {
    let d = kind.def();
    let style = match kind {
        EnemyKind::Slime => RigStyle::Slime,
        EnemyKind::Wolf => RigStyle::Quadruped,
        _ => RigStyle::Humanoid,
    };
    let e = commands
        .spawn((
            Enemy {
                kind,
                hp: d.hp * mult,
                max_hp: d.hp * mult,
                atk: d.atk * mult.sqrt(),
                def: d.def,
                dead: false,
                radius: d.radius,
                state: if kind == EnemyKind::Boss { AiState::Dormant } else { AiState::Idle },
                t: 0.0,
                home: pos,
                vel: Vec3::ZERO,
                facing: if kind == EnemyKind::Boss { PI } else { (pos.x * 7.3).sin() * PI },
                hit_done: false,
                flash: 0.0,
                poise: 0.0,
                cooldown: 1.0,
                mv: Move::Basic,
                wander: pos,
                knock: Vec3::ZERO,
                leap_from: pos,
                leap_to: pos,
                phase2: false,
                summoned: false,
                strafe: 1.0,
                combo: 0,
                last_hit: 99.0,
            },
            Pose { style, two_hand: kind == EnemyKind::Golem, ..default() },
            Transform::from_translation(pos),
            Visibility::default(),
            ZoneEntity,
        ))
        .id();
    match kind {
        EnemyKind::Slime => rig::build_slime(commands, ga, e, 1.0),
        EnemyKind::Wolf => rig::build_quadruped(commands, ga, e, &ga.wolf_fur, &ga.wolf_dark, &ga.red_glow, 1.0),
        EnemyKind::Skeleton => {
            let mut look = Look::basic(ga);
            look.bulk = 0.55;
            look.skin = ga.bone.clone();
            look.torso = ga.bone.clone();
            look.legs = ga.bone.clone();
            look.arms = ga.bone.clone();
            look.boots = ga.rust.clone();
            look.hair = None;
            look.eyes = ga.red_glow.clone();
            look.helmet = Some(ga.rust.clone());
            look.apron = Some(ga.rust.clone());
            look.weapon = Some(WeaponLook {
                blade: ga.rust.clone(),
                edge: ga.rust.clone(),
                hilt: ga.leather.clone(),
                length: 0.9,
                width: 1.1,
            });
            look.shield = Some((ga.rust.clone(), ga.leather.clone()));
            rig::build_humanoid(commands, ga, e, &look);
        }
        EnemyKind::Golem => {
            let mut look = Look::basic(ga);
            look.scale = 1.55;
            look.bulk = 1.9;
            look.skin = ga.golem_stone.clone();
            look.torso = ga.golem_stone.clone();
            look.legs = ga.golem_stone.clone();
            look.arms = ga.golem_moss.clone();
            look.boots = ga.golem_stone.clone();
            look.hair = None;
            look.eyes = ga.cyan_glow.clone();
            look.pauldrons = Some(ga.golem_moss.clone());
            look.apron = Some(ga.cyan_glow.clone());
            rig::build_humanoid(commands, ga, e, &look);
        }
        EnemyKind::Boss => {
            let mut look = Look::basic(ga);
            look.scale = 1.75;
            look.bulk = 1.25;
            look.skin = ga.knight_armor.clone();
            look.torso = ga.knight_armor.clone();
            look.legs = ga.knight_armor.clone();
            look.arms = ga.knight_armor.clone();
            look.boots = ga.knight_trim.clone();
            look.hair = None;
            look.eyes = ga.purple_glow.clone();
            look.helmet = Some(ga.knight_armor.clone());
            look.horns = Some(ga.knight_trim.clone());
            look.pauldrons = Some(ga.knight_trim.clone());
            look.cape = Some(ga.knight_cape.clone());
            look.apron = Some(ga.knight_cape.clone());
            look.weapon = Some(WeaponLook {
                blade: ga.knight_armor.clone(),
                edge: ga.purple_glow.clone(),
                hilt: ga.knight_trim.clone(),
                length: 1.5,
                width: 1.6,
            });
            rig::build_humanoid(commands, ga, e, &look);
            commands.entity(e).insert(Boss);
        }
    }
    e
}

fn spawn_points(
    time: Res<Time>,
    mut commands: Commands,
    ga: Res<GameAssets>,
    zone: Res<CurrentZone>,
    mut points: Query<&mut SpawnPoint>,
    enemies: Query<&Enemy>,
    player: Query<&Transform, With<Player>>,
) {
    let ppos = player.single().map(|t| t.translation).unwrap_or(Vec3::ZERO);
    let mult = if zone.0 == ZoneId::Dungeon { 1.35 } else { 1.0 };
    for mut sp in &mut points {
        if let Some(e) = sp.alive {
            if enemies.get(e).is_err() {
                sp.alive = None;
                sp.timer = 60.0;
            }
            continue;
        }
        if sp.used && !sp.respawn {
            continue;
        }
        sp.timer -= time.delta_secs();
        if sp.timer <= 0.0 && (!sp.used || ppos.distance(sp.pos) > 35.0) {
            sp.alive = Some(spawn_enemy(&mut commands, &ga, sp.kind, sp.pos, mult));
            sp.used = true;
        }
    }
}

fn face_to(from: Vec3, to: Vec3) -> f32 {
    let d = to - from;
    (-d.x).atan2(-d.z)
}

fn fwd(yaw: f32) -> Vec3 {
    Vec3::new(-yaw.sin(), 0.0, -yaw.cos())
}

/// 攻撃ごとのタイミング: (予備動作, 攻撃全体, 判定開始, 判定終了, 硬直)
fn timings(kind: EnemyKind, mv: Move, phase2: bool) -> (f32, f32, f32, f32, f32) {
    let s = if phase2 { 0.8 } else { 1.0 };
    match (kind, mv) {
        (EnemyKind::Slime, _) => (0.55, 0.5, 0.1, 0.4, 0.9),
        (EnemyKind::Wolf, _) => (0.4, 0.45, 0.1, 0.32, 0.7),
        (EnemyKind::Skeleton, _) => (0.55, 0.6, 0.18, 0.34, 0.75),
        (EnemyKind::Golem, _) => (1.0, 0.8, 0.3, 0.42, 1.3),
        (EnemyKind::Boss, Move::Basic | Move::Combo2) => (0.55 * s, 0.62, 0.18, 0.34, 0.5 * s),
        (EnemyKind::Boss, Move::Leap) => (0.6 * s, 1.1, 0.72, 0.82, 0.9 * s),
        (EnemyKind::Boss, Move::Charge) => (0.65 * s, 0.75, 0.05, 0.7, 0.8 * s),
        (EnemyKind::Boss, Move::Orbs) => (0.5, 0.9, 0.35, 0.4, 0.6),
        (EnemyKind::Boss, Move::Summon) => (0.8, 1.0, 0.4, 0.45, 0.5),
    }
}

#[allow(clippy::too_many_arguments)]
fn enemy_ai(
    time: Res<Time>,
    mut commands: Commands,
    ga: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    zone: Res<CurrentZone>,
    player: Query<(Entity, &Transform, &PlayerCtl), With<Player>>,
    mut enemies: Query<(Entity, &mut Transform, &mut Enemy), Without<Player>>,
    mut dmg: MessageWriter<Damage>,
    mut sfx: MessageWriter<PlaySfx>,
    mut cam: ResMut<CamRig>,
) {
    let dt = time.delta_secs();
    let Ok((pe, pt, pctl)) = player.single() else { return };
    let ppos = pt.translation;
    let player_dead = pctl.action == PAction::Dead;
    let mut to_summon = Vec::new();

    for (_e, tf, mut en) in &mut enemies {
        if en.dead {
            continue;
        }
        let def = en.kind.def();
        en.t += dt;
        en.flash = (en.flash - dt * 4.0).max(0.0);
        en.cooldown -= dt;
        en.last_hit += dt;
        let pos = tf.translation;
        let to_p = ppos - pos;
        let dist = Vec2::new(to_p.x, to_p.z).length();
        let boss = en.kind == EnemyKind::Boss;
        let speed = def.speed * if en.phase2 { 1.25 } else { 1.0 };
        let aggro = if en.last_hit < 8.0 { def.aggro * 2.0 } else { def.aggro };

        match en.state {
            AiState::Dormant => {
                en.vel = Vec3::ZERO;
            }
            AiState::Roar => {
                en.vel = Vec3::ZERO;
                en.facing = lerp_angle(en.facing, face_to(pos, ppos), damp(4.0, dt));
                if en.t > 1.8 {
                    en.state = AiState::Chase;
                    en.t = 0.0;
                    en.cooldown = 0.5;
                }
            }
            AiState::Idle | AiState::Wander => {
                if dist < aggro && !player_dead {
                    en.state = AiState::Chase;
                    en.t = 0.0;
                    if en.kind == EnemyKind::Wolf {
                        sfx.write(PlaySfx(SfxKind::Growl));
                    }
                    continue;
                }
                if en.state == AiState::Idle {
                    en.vel *= 1.0 - damp(8.0, dt);
                    if en.t > 2.0 + rng.0.f() * 3.0 {
                        en.state = AiState::Wander;
                        en.t = 0.0;
                        let w = en.home + (rng.0.unit_vec2() * rng.0.range(2.0, 7.0)).extend(0.0).xzy();
                        en.wander = w;
                    }
                } else {
                    let d = en.wander - pos;
                    let d = Vec3::new(d.x, 0.0, d.z);
                    if d.length() < 0.8 || en.t > 6.0 {
                        en.state = AiState::Idle;
                        en.t = 0.0;
                    } else {
                        let dir = d.normalize();
                        en.vel = dir * speed * 0.35;
                        en.facing = lerp_angle(en.facing, face_to(pos, en.wander), damp(5.0, dt));
                    }
                }
            }
            AiState::Chase => {
                let leash = if boss { 999.0 } else { 40.0 };
                if player_dead || (dist > aggro * 1.8 && en.last_hit > 5.0) || pos.distance(en.home) > leash {
                    en.state = AiState::Wander;
                    en.wander = en.home;
                    en.t = 0.0;
                    continue;
                }
                let dir = Vec3::new(to_p.x, 0.0, to_p.z).normalize_or_zero();
                en.facing = lerp_angle(en.facing, face_to(pos, ppos), damp(8.0, dt));
                // 攻撃の選択
                if en.cooldown <= 0.0 {
                    let mut chosen = None;
                    if boss {
                        let r = rng.0.f();
                        if en.phase2 && !en.summoned {
                            chosen = Some(Move::Summon);
                        } else if dist > 9.0 && r < 0.55 {
                            chosen = Some(Move::Leap);
                        } else if dist > 6.0 && r < 0.8 {
                            chosen = Some(if en.phase2 && r < 0.4 { Move::Orbs } else { Move::Charge });
                        } else if dist < def.range {
                            chosen = Some(if rng.0.chance(0.55) { Move::Combo2 } else { Move::Basic });
                        } else if en.phase2 && r > 0.93 {
                            chosen = Some(Move::Orbs);
                        }
                    } else if dist < def.range {
                        chosen = Some(Move::Basic);
                    }
                    if let Some(mv) = chosen {
                        en.mv = mv;
                        en.state = AiState::Windup;
                        en.t = 0.0;
                        en.combo = 0;
                        en.vel = Vec3::ZERO;
                        if boss {
                            sfx.write(PlaySfx(SfxKind::BossGrunt));
                        }
                        // 予兆の赤い円
                        let (wind, _, _, _, _) = timings(en.kind, mv, en.phase2);
                        let warn = match (en.kind, mv) {
                            (EnemyKind::Golem, _) => Some((pos + fwd(en.facing) * 2.2, 3.4, wind)),
                            (EnemyKind::Boss, Move::Leap) => Some((ppos, 4.8, wind + 0.8)),
                            _ => None,
                        };
                        if let Some((c, r, life)) = warn {
                            spawn_warn(&mut commands, &ga, zone.0, c, r, life);
                        }
                        continue;
                    }
                }
                // 移動
                let keep = if en.kind == EnemyKind::Wolf && en.cooldown > 0.0 { 5.0 } else { def.range * 0.7 };
                let side = Vec3::new(-dir.z, 0.0, dir.x) * en.strafe;
                let desired = if dist > keep {
                    dir * speed
                } else if en.kind == EnemyKind::Wolf || boss {
                    side * speed * 0.55
                } else {
                    Vec3::ZERO
                };
                if rng.0.chance(dt * 0.3) {
                    en.strafe = -en.strafe;
                }
                en.vel = en.vel.lerp(desired, damp(6.0, dt));
            }
            AiState::Windup => {
                let (wind, _, _, _, _) = timings(en.kind, en.mv, en.phase2);
                en.vel *= 1.0 - damp(10.0, dt);
                // 予備動作中は相手の方へ向き直る
                let turn = if en.kind == EnemyKind::Golem { 3.0 } else { 9.0 };
                en.facing = lerp_angle(en.facing, face_to(pos, ppos), damp(turn, dt));
                if en.t >= wind {
                    en.state = AiState::Attack;
                    en.t = 0.0;
                    en.hit_done = false;
                    match en.mv {
                        Move::Leap => {
                            en.leap_from = pos;
                            let target = ppos;
                            en.leap_to = target - Vec3::new(to_p.x, 0.0, to_p.z).normalize_or_zero() * 1.2;
                            sfx.write(PlaySfx(SfxKind::Jump));
                        }
                        Move::Charge => {
                            sfx.write(PlaySfx(SfxKind::SwingHeavy));
                        }
                        Move::Orbs => {
                            sfx.write(PlaySfx(SfxKind::Magic));
                        }
                        _ => {
                            if en.kind != EnemyKind::Slime {
                                sfx.write(PlaySfx(if en.kind == EnemyKind::Golem || boss { SfxKind::SwingHeavy } else { SfxKind::Swing }));
                            } else {
                                sfx.write(PlaySfx(SfxKind::Slime));
                            }
                        }
                    }
                }
            }
            AiState::Attack => {
                let (_, dur, hs, he, rec) = timings(en.kind, en.mv, en.phase2);
                let f = fwd(en.facing);
                let active = en.t >= hs && en.t - dt <= he;
                match (en.kind, en.mv) {
                    (EnemyKind::Slime, _) => {
                        en.vel = if en.t < 0.35 { f * 7.5 } else { en.vel * 0.8 };
                    }
                    (EnemyKind::Wolf, _) => {
                        en.vel = if en.t < 0.3 { f * 13.0 } else { en.vel * 0.8 };
                    }
                    (EnemyKind::Boss, Move::Charge) => {
                        en.vel = if en.t < 0.7 { f * 17.0 } else { en.vel * 0.8 };
                        if rng.0.chance(0.6) {
                            burst(&mut commands, &ga, &mut rng.0, Burst::Step, pos);
                        }
                    }
                    (EnemyKind::Boss, Move::Leap) => {
                        en.vel = Vec3::ZERO;
                    }
                    _ => {
                        en.vel = if en.t < hs { f * 2.5 } else { en.vel * 0.85 };
                    }
                }
                if active && !en.hit_done && !player_dead {
                    let (reach, arc, mult, knock, aoe) = match (en.kind, en.mv) {
                        (EnemyKind::Slime, _) => (def.radius + 0.9, PI, 1.0, 5.0, false),
                        (EnemyKind::Wolf, _) => (1.7, 1.2, 1.0, 5.0, false),
                        (EnemyKind::Skeleton, _) => (2.5, 1.1, 1.0, 5.0, false),
                        (EnemyKind::Golem, _) => (3.4, PI, 1.3, 11.0, true),
                        (EnemyKind::Boss, Move::Leap) => (4.8, PI, 1.5, 13.0, true),
                        (EnemyKind::Boss, Move::Charge) => (2.4, 1.2, 1.3, 12.0, false),
                        (EnemyKind::Boss, Move::Orbs | Move::Summon) => (0.0, 0.0, 0.0, 0.0, false),
                        (EnemyKind::Boss, _) => (4.3, 1.3, 1.1, 8.0, false),
                    };
                    let center = if aoe {
                        if en.mv == Move::Leap { en.leap_to } else { pos + f * 2.2 }
                    } else {
                        pos
                    };
                    let d = ppos - center;
                    let flat = Vec3::new(d.x, 0.0, d.z);
                    let within = flat.length() < reach && d.y.abs() < 2.5;
                    let ang_ok = aoe || flat.normalize_or_zero().dot(f) > arc.cos() || flat.length() < 1.0;
                    if aoe && en.t >= hs && !en.hit_done {
                        // 着地の衝撃波
                        burst(&mut commands, &ga, &mut rng.0, Burst::Shock(reach), Vec3::new(center.x, ground_height(zone.0, center.xz()) + 0.1, center.z));
                        sfx.write(PlaySfx(SfxKind::Slam));
                        cam.shake(if boss { 0.8 } else { 0.55 });
                    }
                    if within && ang_ok && reach > 0.0 {
                        en.hit_done = true;
                        dmg.write(Damage {
                            target: pe,
                            amount: en.atk * mult,
                            crit: false,
                            from: center,
                            knock,
                            heavy: aoe,
                        });
                    } else if aoe {
                        en.hit_done = true;
                    }
                }
                if en.mv == Move::Orbs && en.t >= hs && !en.hit_done {
                    en.hit_done = true;
                    let n = if en.phase2 { 5 } else { 3 };
                    for i in 0..n {
                        let a = en.facing + (i as f32 - (n - 1) as f32 / 2.0) * 0.5;
                        let dir = fwd(a);
                        spawn_orb(&mut commands, &ga, pos + Vec3::Y * 3.5 + dir * 1.0, dir * 6.0 + Vec3::Y * 2.0, en.atk * 0.7);
                    }
                }
                if en.mv == Move::Summon && en.t >= hs && !en.hit_done {
                    en.hit_done = true;
                    en.summoned = true;
                    sfx.write(PlaySfx(SfxKind::Magic));
                    for s in [-1.0, 1.0] {
                        let p = pos + Vec3::new(s * 4.0, 0.0, 2.0);
                        to_summon.push(p);
                        burst(&mut commands, &ga, &mut rng.0, Burst::Death, p + Vec3::Y);
                    }
                }
                if en.t >= dur {
                    // ボスの連撃 / スケルトンの二段斬り
                    let chain = (en.mv == Move::Combo2 && en.combo == 0)
                        || (en.kind == EnemyKind::Skeleton && en.combo == 0 && rng.0.chance(0.35));
                    if chain {
                        en.combo = 1;
                        en.state = AiState::Attack;
                        en.t = 0.0;
                        en.hit_done = false;
                        en.facing = lerp_angle(en.facing, face_to(pos, ppos), 0.7);
                        sfx.write(PlaySfx(SfxKind::Swing));
                    } else {
                        en.state = AiState::Recover;
                        en.t = 0.0;
                        en.cooldown = rec + rng.0.range(0.2, 1.2) * if boss { 0.5 } else { 1.0 };
                    }
                }
            }
            AiState::Recover => {
                en.vel *= 1.0 - damp(6.0, dt);
                let (_, _, _, _, rec) = timings(en.kind, en.mv, en.phase2);
                if en.t > rec {
                    en.state = AiState::Chase;
                    en.t = 0.0;
                }
            }
            AiState::Hurt => {
                en.vel *= 1.0 - damp(6.0, dt);
                let stun = if boss { 1.2 } else if en.kind == EnemyKind::Golem { 0.8 } else { 0.35 };
                if en.t > stun {
                    en.state = AiState::Chase;
                    en.t = 0.0;
                    en.cooldown = en.cooldown.min(0.3);
                }
            }
            AiState::Dying => {}
        }

        // ボス第二形態
        if boss && !en.phase2 && en.hp < en.max_hp * 0.5 {
            en.phase2 = true;
            en.state = AiState::Roar;
            en.t = 0.0;
            sfx.write(PlaySfx(SfxKind::Roar));
            cam.shake(0.6);
            burst(&mut commands, &ga, &mut rng.0, Burst::Shock(7.0), pos + Vec3::Y * 0.1);
        }
    }
    for p in to_summon {
        commands.spawn((SpawnPoint::new(EnemyKind::Skeleton, p).no_respawn(), ZoneEntity));
    }
}

fn spawn_warn(commands: &mut Commands, ga: &GameAssets, zone: ZoneId, c: Vec3, r: f32, life: f32) {
    let y = ground_height(zone, c.xz()) + 0.08;
    commands.spawn((
        Mesh3d(ga.disc.clone()),
        MeshMaterial3d(ga.fx_warn.clone()),
        Transform::from_translation(Vec3::new(c.x, y, c.z))
            .with_rotation(Quat::from_rotation_x(-PI / 2.0))
            .with_scale(Vec3::splat(r * 0.3)),
        Timed::new(life).grow(Vec3::splat(r * 0.3), Vec3::splat(r)),
        NotShadowCaster,
        ZoneEntity,
    ));
    commands.spawn((
        Mesh3d(ga.ring.clone()),
        MeshMaterial3d(ga.fx_warn.clone()),
        Transform::from_translation(Vec3::new(c.x, y + 0.01, c.z))
            .with_rotation(Quat::from_rotation_x(-PI / 2.0))
            .with_scale(Vec3::splat(r)),
        Timed::new(life),
        NotShadowCaster,
        ZoneEntity,
    ));
}

fn spawn_orb(commands: &mut Commands, ga: &GameAssets, pos: Vec3, vel: Vec3, dmg: f32) {
    commands.spawn((
        Mesh3d(ga.sphere.clone()),
        MeshMaterial3d(ga.fx_purple.clone()),
        Transform::from_translation(pos).with_scale(Vec3::splat(0.45)),
        Orb { vel, life: 6.0, dmg },
        NotShadowCaster,
        ZoneEntity,
    ));
}

#[allow(clippy::too_many_arguments)]
fn orbs(
    time: Res<Time>,
    mut commands: Commands,
    ga: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    zone: Res<CurrentZone>,
    colliders: Res<Colliders>,
    player: Query<(Entity, &Transform, &PlayerCtl), With<Player>>,
    mut q: Query<(Entity, &mut Transform, &mut Orb), Without<Player>>,
    mut dmg: MessageWriter<Damage>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let dt = time.delta_secs();
    let Ok((pe, pt, _)) = player.single() else { return };
    let target = pt.translation + Vec3::Y * 1.0;
    for (e, mut tf, mut orb) in &mut q {
        orb.life -= dt;
        let want = (target - tf.translation).normalize_or_zero() * 7.5;
        let k = if orb.life > 5.2 { 0.5 } else { 2.2 };
        orb.vel = orb.vel.lerp(want, damp(k, dt));
        tf.translation += orb.vel * dt;
        tf.scale = Vec3::splat(0.45 + (orb.life * 12.0).sin() * 0.05);
        if rng.0.chance(0.5) {
            burst(&mut commands, &ga, &mut rng.0, Burst::Trail, tf.translation);
        }
        let hit_wall = colliders.grid.as_ref().map(|g| g.wall_at(tf.translation.xz())).unwrap_or(false)
            || tf.translation.y < ground_height(zone.0, tf.translation.xz());
        if tf.translation.distance(target) < 0.9 {
            dmg.write(Damage { target: pe, amount: orb.dmg, crit: false, from: tf.translation, knock: 5.0, heavy: false });
            orb.life = 0.0;
        }
        if orb.life <= 0.0 || hit_wall {
            burst(&mut commands, &ga, &mut rng.0, Burst::Magic, tf.translation);
            sfx.write(PlaySfx(SfxKind::Hit));
            commands.entity(e).despawn();
        }
    }
}

fn enemy_physics(
    time: Res<Time>,
    zone: Res<CurrentZone>,
    colliders: Res<Colliders>,
    mut enemies: Query<(Entity, &mut Transform, &mut Enemy), Without<Player>>,
    mut player: Query<&mut Transform, With<Player>>,
) {
    let dt = time.delta_secs().min(0.05);
    let z = zone.0;
    let snapshot: Vec<(Entity, Vec2, f32)> = enemies
        .iter()
        .filter(|(_, _, e)| !e.dead)
        .map(|(e, t, en)| (e, t.translation.xz(), en.radius))
        .collect();
    let ppos = player.single().map(|t| t.translation.xz()).unwrap_or(Vec2::ZERO);
    let mut push_player = Vec2::ZERO;
    for (e, mut tf, mut en) in &mut enemies {
        let knock = en.knock;
        en.knock = knock * (1.0 - damp(7.0, dt));
        if en.dead {
            continue;
        }
        let old = tf.translation.xz();
        let mut p = old;
        if en.state == AiState::Attack && en.mv == Move::Leap {
            let (_, dur, hs, _, _) = timings(en.kind, en.mv, en.phase2);
            let u = (en.t / hs).min(1.0);
            let from = en.leap_from;
            let to = en.leap_to;
            let h = from.lerp(to, u);
            tf.translation.x = h.x;
            tf.translation.z = h.z;
            tf.translation.y = ground_height(z, h.xz()) + (u * PI).sin() * 7.0;
            let _ = dur;
            tf.rotation = Quat::from_rotation_y(en.facing);
            continue;
        }
        p += (en.vel.xz() + knock.xz()) * dt;
        // 敵同士の押し合い
        for (o, op, or) in &snapshot {
            if *o == e {
                continue;
            }
            let d = p - *op;
            let min = en.radius + or;
            let l = d.length();
            if l < min && l > 1e-4 {
                p += d / l * (min - l) * 0.5;
            }
        }
        // プレイヤーとの押し合い
        let d = ppos - p;
        let min = en.radius + 0.4;
        let l = d.length();
        if l < min && l > 1e-4 {
            let pen = min - l;
            p -= d / l * pen * 0.3;
            push_player += d / l * pen * 0.7;
        }
        p = colliders.resolve(p, en.radius);
        if blocked(z, p) {
            p = old;
        }
        tf.translation.x = p.x;
        tf.translation.z = p.y;
        let g = ground_height(z, p);
        tf.translation.y += (g - tf.translation.y) * damp(20.0, dt);
        tf.rotation = Quat::from_rotation_y(en.facing);
        let pop = 1.0 + en.flash * 0.12;
        tf.scale = Vec3::splat(pop);
    }
    if push_player != Vec2::ZERO
        && let Ok(mut pt) = player.single_mut()
    {
        let np = colliders.resolve(pt.translation.xz() + push_player, 0.4);
        if !blocked(z, np) {
            pt.translation.x = np.x;
            pt.translation.z = np.y;
        }
    }
}

fn enemy_pose(time: Res<Time>, mut q: Query<(&Enemy, &mut Pose)>) {
    let dt = time.delta_secs();
    for (en, mut pose) in &mut q {
        let def = en.kind.def();
        let hs = en.vel.xz().length();
        pose.clock += dt;
        pose.phase += hs * dt * if en.kind == EnemyKind::Wolf { 0.9 } else { 1.6 } / en.kind_scale();
        pose.moving += ((hs / def.speed.max(1.0)).min(1.0) - pose.moving) * damp(8.0, dt);
        pose.hurt = if en.state == AiState::Hurt { (1.0 - en.t / 0.35).max(0.0) } else { 0.0 };
        let (wind, dur, _, _, _) = timings(en.kind, en.mv, en.phase2);
        pose.charge = 0.0;
        pose.air = false;
        let (action, t) = match en.state {
            AiState::Windup => {
                pose.charge = (en.t / wind).min(1.0);
                (PoseAction::Windup, pose.charge)
            }
            AiState::Attack => {
                let t = (en.t / dur).min(1.0);
                let a = match (en.kind, en.mv) {
                    (EnemyKind::Slime, _) => PoseAction::Slam,
                    (EnemyKind::Wolf, _) => PoseAction::Bite,
                    (EnemyKind::Golem, _) => PoseAction::Slam,
                    (EnemyKind::Boss, Move::Leap) => PoseAction::Leap,
                    (EnemyKind::Boss, Move::Charge) => PoseAction::Thrust,
                    (EnemyKind::Boss, Move::Orbs | Move::Summon) => PoseAction::Cast,
                    _ => {
                        if en.combo == 1 {
                            PoseAction::Slash2
                        } else {
                            PoseAction::Slash1
                        }
                    }
                };
                if en.mv == Move::Leap {
                    pose.air = true;
                }
                (a, t)
            }
            AiState::Roar => (PoseAction::Windup, 1.0),
            _ => (PoseAction::None, 0.0),
        };
        if en.state == AiState::Roar {
            pose.charge = 1.0;
        }
        pose.action = action;
        pose.t = t;
        pose.dead = if en.state == AiState::Dying { (en.t * 1.5).min(1.0) } else { 0.0 };
    }
}

impl Enemy {
    fn kind_scale(&self) -> f32 {
        match self.kind {
            EnemyKind::Golem => 1.55,
            EnemyKind::Boss => 1.75,
            _ => 1.0,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn enemy_dying(
    time: Res<Time>,
    mut commands: Commands,
    ga: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut q: Query<(Entity, &mut Transform, &mut Enemy)>,
) {
    let dt = time.delta_secs();
    for (e, mut tf, mut en) in &mut q {
        if en.state != AiState::Dying {
            continue;
        }
        en.t += dt;
        if en.t > 0.6 {
            let s = (1.0 - (en.t - 0.6) / 0.5).max(0.0);
            tf.scale = Vec3::new(1.0 + (1.0 - s) * 0.3, s.max(0.01), 1.0 + (1.0 - s) * 0.3);
        }
        if en.t > 1.1 {
            burst(&mut commands, &ga, &mut rng.0, Burst::Death, tf.translation + Vec3::Y * 0.8);
            commands.entity(e).despawn();
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn boss_arena(
    mut commands: Commands,
    ga: Res<GameAssets>,
    zone: Res<CurrentZone>,
    mut colliders: ResMut<Colliders>,
    mut arenas: Query<&mut BossArena>,
    mut bosses: Query<&mut Enemy, With<Boss>>,
    gates: Query<Entity, With<BossGate>>,
    player: Query<&Transform, With<Player>>,
    mut killed: MessageReader<EnemyKilled>,
    mut music: ResMut<Music>,
    mut banner: MessageWriter<Banner>,
    mut sfx: MessageWriter<PlaySfx>,
    mut progress: ResMut<Progress>,
    mut cam: ResMut<CamRig>,
) {
    if zone.0 != ZoneId::Dungeon {
        killed.clear();
        return;
    }
    let Ok(pt) = player.single() else { return };
    for mut arena in &mut arenas {
        if arena.active {
            continue;
        }
        if pt.translation.distance(ARENA_CENTER) < ARENA_RADIUS - 5.0 {
            arena.active = true;
            for mut b in &mut bosses {
                b.state = AiState::Roar;
                b.t = 0.0;
                b.last_hit = 0.0;
            }
            *music = Music::Boss;
            banner.write(Banner::big("黒騎士ヴァルガ", "古代遺跡の守護者"));
            sfx.write(PlaySfx(SfxKind::Roar));
            cam.shake(0.5);
            // 入口を封鎖
            let gate = Obb { center: GATE_CENTER, half: Vec2::new(6.0, 0.6), angle: 0.0 };
            colliders.dynamic_boxes.push(gate);
            for i in 0..7 {
                let x = GATE_CENTER.x - 5.4 + i as f32 * 1.8;
                commands.spawn((
                    Mesh3d(ga.cylinder.clone()),
                    MeshMaterial3d(ga.knight_trim.clone()),
                    Transform::from_xyz(x, 3.0, GATE_CENTER.y).with_scale(Vec3::new(0.25, 6.0, 0.25)),
                    BossGate,
                    ZoneEntity,
                ));
            }
        }
    }
    for k in killed.read() {
        if k.kind == EnemyKind::Boss {
            progress.boss_defeated = true;
            if progress.main == MainQuest::GoToRuins || progress.main == MainQuest::NotStarted {
                progress.main = MainQuest::ReturnToElder;
            }
            colliders.dynamic_boxes.clear();
            for g in &gates {
                commands.entity(g).despawn();
            }
            *music = Music::Victory;
            banner.write(Banner::big("黒騎士ヴァルガを倒した!", "長老に報告しよう"));
            dungeon::spawn_portal(&mut commands, &ga, ARENA_CENTER + Vec3::Z * 3.0);
            let _ = TAU;
        }
    }
}
