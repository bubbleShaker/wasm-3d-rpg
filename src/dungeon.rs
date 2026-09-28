//! ダンジョン「黒騎士の古代遺跡」の生成。

use std::f32::consts::TAU;

use bevy::{light::NotShadowCaster, prelude::*};

use crate::{
    assets::GameAssets,
    enemy::{BossArena, EnemyKind, SpawnPoint},
    field::SpawnCtx,
    npc::{InteractKind, Interactable, spawn_chest},
    util::{LpMesh, Rng},
    vfx::{Emitter, EmitterKind, Flicker, Spin},
    zone::{Colliders, TileGrid, ZoneEntity},
};

pub const TILE: f32 = 4.0;
const W: i32 = 40;
const H: i32 = 40;
const WALL_H: f32 = 6.0;

pub fn tile_center(x: f32, z: f32) -> Vec3 {
    Vec3::new((x + 0.5) * TILE, 0.0, (z + 0.5) * TILE)
}

pub const DUNGEON_SPAWN: Vec3 = Vec3::new(6.5 * TILE, 0.0, 34.0 * TILE);
pub const ARENA_CENTER: Vec3 = Vec3::new(20.5 * TILE, 0.0, 6.2 * TILE);
pub const ARENA_RADIUS: f32 = 5.3 * TILE;
/// ボス部屋入口の封鎖位置
pub const GATE_CENTER: Vec2 = Vec2::new(20.5 * TILE, 12.5 * TILE);

fn build_grid() -> TileGrid {
    let mut walls = vec![true; (W * H) as usize];
    let mut carve = |x0: i32, z0: i32, x1: i32, z1: i32| {
        for z in z0..z1 {
            for x in x0..x1 {
                if x > 0 && z > 0 && x < W - 1 && z < H - 1 {
                    walls[(z * W + x) as usize] = false;
                }
            }
        }
    };
    carve(3, 32, 10, 38); // 入口の間
    carve(10, 34, 16, 36); // 通路
    carve(16, 30, 26, 38); // 部屋A
    carve(19, 22, 22, 30); // 北へ
    carve(12, 14, 30, 22); // 柱の大広間
    carve(30, 17, 33, 19); // 東の通路
    carve(33, 13, 39, 22); // 宝物庫
    carve(6, 17, 12, 19); // 西の通路
    carve(2, 12, 7, 25); // 西の回廊
    carve(19, 10, 22, 14); // ボス部屋への通路
    let mut g = TileGrid { w: W, h: H, size: TILE, walls };
    // 円形のボス部屋
    let c = Vec2::new(ARENA_CENTER.x, ARENA_CENTER.z) / TILE;
    for z in 1..H - 1 {
        for x in 1..W - 1 {
            let p = Vec2::new(x as f32 + 0.5, z as f32 + 0.5);
            if p.distance(c) < 5.4 {
                g.walls[(z * W + x) as usize] = false;
            }
        }
    }
    g
}

/// 大広間とボス部屋の丸柱 (タイルではなく円の当たり判定)。
fn pillars() -> Vec<Vec2> {
    let mut v = Vec::new();
    for x in [14.0, 17.5, 24.5, 28.0] {
        for z in [15.8, 19.8] {
            v.push(Vec2::new(x, z) * TILE);
        }
    }
    for i in 0..6 {
        let a = i as f32 / 6.0 * TAU + TAU / 12.0;
        v.push(Vec2::new(ARENA_CENTER.x, ARENA_CENTER.z) + Vec2::from_angle(a) * 14.5);
    }
    v
}

fn build_meshes(g: &TileGrid) -> (Mesh, Mesh) {
    let mut rng = Rng::new(4242);
    let mut lp = LpMesh::default();
    let mut floor = LpMesh::default();
    for z in 0..H {
        for x in 0..W {
            let x0 = x as f32 * TILE;
            let z0 = z as f32 * TILE;
            if g.is_wall(x, z) {
                let near_floor = (-1..=1).any(|dz| (-1..=1).any(|dx| !g.is_wall(x + dx, z + dz)));
                if !near_floor {
                    continue;
                }
                // 3 段に積んだ石ブロック
                let mut y = 0.0;
                for layer in 0..3 {
                    let lh = WALL_H / 3.0;
                    let g0 = rng.range(0.24, 0.32);
                    let colr = Color::srgb(g0, g0 * 0.95, g0 * 1.02);
                    let inset = if layer == 1 { 0.08 } else { 0.0 };
                    lp.cuboid(
                        Vec3::new(x0 + TILE / 2.0, y + lh / 2.0, z0 + TILE / 2.0),
                        Vec3::new(TILE / 2.0 - inset, lh / 2.0, TILE / 2.0 - inset),
                        Quat::IDENTITY,
                        colr,
                    );
                    y += lh;
                }
                lp.cuboid(
                    Vec3::new(x0 + TILE / 2.0, WALL_H + 0.1, z0 + TILE / 2.0),
                    Vec3::new(TILE / 2.0 + 0.05, 0.12, TILE / 2.0 + 0.05),
                    Quat::IDENTITY,
                    Color::srgb(0.18, 0.17, 0.2),
                );
            } else {
                // 床: 2x2 の敷石 + 目地
                floor.quad(
                    Vec3::new(x0, -0.01, z0),
                    Vec3::new(x0, -0.01, z0 + TILE),
                    Vec3::new(x0 + TILE, -0.01, z0 + TILE),
                    Vec3::new(x0 + TILE, -0.01, z0),
                    Color::srgb(0.08, 0.075, 0.08),
                );
                let s = TILE / 2.0;
                for i in 0..2 {
                    for k in 0..2 {
                        let g0 = rng.range(0.3, 0.42);
                        let colr = Color::srgb(g0, g0 * 0.96, g0 * 0.9);
                        let a = Vec3::new(x0 + i as f32 * s + 0.06, rng.range(0.0, 0.03), z0 + k as f32 * s + 0.06);
                        let e = s - 0.12;
                        floor.quad(a, a + Vec3::Z * e, a + Vec3::new(e, 0.0, e), a + Vec3::X * e, colr);
                    }
                }
            }
        }
    }
    // 丸柱
    for p in pillars() {
        let seg = 8;
        for layer in 0..3 {
            let y0 = layer as f32 * 2.0;
            let r = if layer == 0 { 1.25 } else { 1.0 };
            let g0 = rng.range(0.32, 0.4);
            for i in 0..seg {
                let a0 = i as f32 / seg as f32 * TAU;
                let a1 = (i + 1) as f32 / seg as f32 * TAU;
                let v = |a: f32, y: f32| Vec3::new(p.x + a.cos() * r, y, p.y + a.sin() * r);
                lp.quad(v(a0, y0), v(a0, y0 + 2.0), v(a1, y0 + 2.0), v(a1, y0), Color::srgb(g0, g0 * 0.95, g0 * 0.9));
            }
        }
    }
    (lp.build(), floor.build())
}

pub fn spawn_dungeon(commands: &mut Commands, ctx: &mut SpawnCtx, col: &mut Colliders) {
    let grid = build_grid();
    let (walls, floor) = if let Some(m) = &ctx.cache.dungeon {
        m.clone()
    } else {
        let (w, f) = build_meshes(&grid);
        let m = (ctx.meshes.add(w), ctx.meshes.add(f));
        ctx.cache.dungeon = Some(m.clone());
        m
    };
    commands.spawn((Mesh3d(walls), MeshMaterial3d(ctx.ga.vcol.clone()), ZoneEntity));
    let floor_mat = ctx.mats.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.55,
        reflectance: 0.35,
        ..default()
    });
    commands.spawn((Mesh3d(floor), MeshMaterial3d(floor_mat), ZoneEntity));
    for p in pillars() {
        col.circles.push((p, 1.3));
    }

    let ga = &ctx.ga;
    // 松明
    let mut rng = Rng::new(77);
    let mut torches: Vec<Vec2> = Vec::new();
    let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    for z in 0..H {
        for x in 0..W {
            if grid.is_wall(x, z) {
                continue;
            }
            for (dx, dz) in dirs {
                if !grid.is_wall(x + dx, z + dz) {
                    continue;
                }
                let c = tile_center(x as f32, z as f32);
                let p = c.xz() + Vec2::new(dx as f32, dz as f32) * (TILE / 2.0 - 0.25);
                if torches.iter().any(|t| t.distance(p) < 15.0) || torches.len() >= 26 {
                    continue;
                }
                if !rng.chance(0.5) {
                    continue;
                }
                torches.push(p);
                let pos = Vec3::new(p.x, 3.0, p.y);
                commands.spawn((
                    Mesh3d(ga.cylinder.clone()),
                    MeshMaterial3d(ga.leather.clone()),
                    Transform::from_translation(pos - Vec3::Y * 0.35)
                        .with_rotation(Quat::from_rotation_arc(Vec3::Y, Vec3::new(-dx as f32 * 0.4, 1.0, -dz as f32 * 0.4).normalize()))
                        .with_scale(Vec3::new(0.14, 0.8, 0.14)),
                    ZoneEntity,
                ));
                commands.spawn((
                    Mesh3d(ga.cone.clone()),
                    MeshMaterial3d(ga.fx_fire.clone()),
                    Transform::from_translation(pos + Vec3::Y * 0.2 - Vec3::new(dx as f32, 0.0, dz as f32) * 0.15)
                        .with_scale(Vec3::new(0.3, 0.55, 0.3)),
                    Flicker { base: Vec3::new(0.3, 0.55, 0.3), speed: 13.0, amount: 0.3, light_base: 0.0 },
                    NotShadowCaster,
                    ZoneEntity,
                ));
                let lp = pos + Vec3::Y * 0.4 - Vec3::new(dx as f32, 0.0, dz as f32) * 0.8;
                commands.spawn((
                    PointLight {
                        color: Color::srgb(1.0, 0.55, 0.25),
                        intensity: 160_000.0,
                        range: 13.0,
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(lp),
                    Flicker { base: Vec3::ONE, speed: 10.0 + rng.range(0.0, 3.0), amount: 0.25, light_base: 160_000.0 },
                    ZoneEntity,
                ));
                commands.spawn((
                    Transform::from_translation(pos + Vec3::Y * 0.4),
                    Emitter::new(EmitterKind::Embers, 0.3),
                    ZoneEntity,
                ));
            }
        }
    }

    // 帰還ポータル
    spawn_portal(commands, &ctx.ga, Vec3::new(6.5 * TILE, 0.0, 36.6 * TILE));

    // ボス部屋の魔法陣
    let ga = &ctx.ga;
    commands.spawn((
        Mesh3d(ga.ring.clone()),
        MeshMaterial3d(ga.fx_purple.clone()),
        Transform::from_translation(ARENA_CENTER + Vec3::Y * 0.05)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::splat(7.5)),
        Spin { axis: Vec3::Z, speed: 0.2, bob: 0.0 },
        NotShadowCaster,
        ZoneEntity,
    ));
    commands.spawn((
        Mesh3d(ga.ring.clone()),
        MeshMaterial3d(ga.fx_purple.clone()),
        Transform::from_translation(ARENA_CENTER + Vec3::Y * 0.05)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::splat(4.5)),
        Spin { axis: Vec3::Z, speed: -0.35, bob: 0.0 },
        NotShadowCaster,
        ZoneEntity,
    ));
    for i in 0..4 {
        let a = i as f32 / 4.0 * TAU;
        let p = ARENA_CENTER + Vec3::new(a.cos() * 9.0, 0.0, a.sin() * 9.0);
        commands.spawn((
            Mesh3d(ga.cylinder.clone()),
            MeshMaterial3d(ga.knight_armor.clone()),
            Transform::from_translation(p + Vec3::Y * 0.6).with_scale(Vec3::new(1.0, 1.2, 1.0)),
            ZoneEntity,
        ));
        commands.spawn((
            Mesh3d(ga.cone.clone()),
            MeshMaterial3d(ga.fx_purple.clone()),
            Transform::from_translation(p + Vec3::Y * 1.6).with_scale(Vec3::new(0.7, 1.0, 0.7)),
            Flicker { base: Vec3::new(0.7, 1.0, 0.7), speed: 8.0, amount: 0.3, light_base: 0.0 },
            NotShadowCaster,
            ZoneEntity,
        ));
        commands.spawn((
            PointLight {
                color: Color::srgb(0.7, 0.3, 1.0),
                intensity: 200_000.0,
                range: 16.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(p + Vec3::Y * 2.5),
            Flicker { base: Vec3::ONE, speed: 7.0, amount: 0.2, light_base: 200_000.0 },
            ZoneEntity,
        ));
        col.circles.push((p.xz(), 1.0));
    }

    // 宝箱
    spawn_chest(commands, ga, 10, tile_center(36.0, 14.0), std::f32::consts::PI, ctx.progress.chest_opened(10));
    spawn_chest(commands, ga, 11, tile_center(3.0, 12.5), std::f32::consts::PI, ctx.progress.chest_opened(11));
    col.circles.push((tile_center(36.0, 14.0).xz(), 0.8));
    col.circles.push((tile_center(3.0, 12.5).xz(), 0.8));

    // 敵
    let e = |c: &mut Commands, k: EnemyKind, x: f32, z: f32| {
        c.spawn((SpawnPoint::new(k, tile_center(x, z)).no_respawn(), ZoneEntity));
    };
    e(commands, EnemyKind::Skeleton, 19.0, 32.0);
    e(commands, EnemyKind::Skeleton, 22.5, 35.5);
    e(commands, EnemyKind::Slime, 24.0, 31.0);
    e(commands, EnemyKind::Skeleton, 15.0, 18.0);
    e(commands, EnemyKind::Golem, 21.0, 17.5);
    e(commands, EnemyKind::Skeleton, 26.0, 20.0);
    e(commands, EnemyKind::Slime, 13.0, 15.0);
    e(commands, EnemyKind::Slime, 28.5, 15.0);
    e(commands, EnemyKind::Golem, 35.5, 18.0);
    e(commands, EnemyKind::Skeleton, 4.0, 14.0);
    e(commands, EnemyKind::Skeleton, 4.5, 22.0);
    e(commands, EnemyKind::Wolf, 3.5, 18.0);

    if ctx.progress.boss_defeated {
        spawn_portal(commands, &ctx.ga, ARENA_CENTER + Vec3::Z * 3.0);
    } else {
        commands.spawn((SpawnPoint::new(EnemyKind::Boss, ARENA_CENTER - Vec3::Z * 4.0).no_respawn(), ZoneEntity));
        commands.spawn((BossArena { active: false }, ZoneEntity));
    }

    col.grid = Some(grid);
}

pub fn spawn_portal(commands: &mut Commands, ga: &GameAssets, pos: Vec3) {
    let center = pos + Vec3::Y * 2.4;
    commands.spawn((
        Mesh3d(ga.torus.clone()),
        MeshMaterial3d(ga.fx_blue.clone()),
        Transform::from_translation(center)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::new(1.8, 0.5, 1.8)),
        NotShadowCaster,
        Interactable { kind: InteractKind::ReturnPortal, radius: 3.2 },
        ZoneEntity,
    ));
    commands.spawn((
        Mesh3d(ga.disc.clone()),
        MeshMaterial3d(ga.fx_trail.clone()),
        Transform::from_translation(center).with_scale(Vec3::splat(1.55)),
        Spin { axis: Vec3::Z, speed: 2.0, bob: 0.0 },
        NotShadowCaster,
        ZoneEntity,
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(0.4, 0.7, 1.0),
            intensity: 200_000.0,
            range: 12.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(center + Vec3::Y * 0.5),
        ZoneEntity,
    ));
    commands.spawn((
        Transform::from_translation(center),
        Emitter::new(EmitterKind::Sparkle, 0.05),
        ZoneEntity,
    ));
}
