//! フィールド (村・森・湖・遺跡) の生成。

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::{ecs::system::SystemParam, light::NotShadowCaster, prelude::*};

use crate::{
    assets::{GameAssets, glow},
    enemy::{EnemyKind, SpawnPoint},
    npc::{InteractKind, Interactable, NpcId, spawn_chest, spawn_npc},
    stats::Progress,
    terrain::{self, LAKE, RUINS, RUINS_H, WATER_Y, height, path_distance},
    util::{LpMesh, Rng, fbm},
    vfx::{Emitter, EmitterKind, Flicker, Spin, Sway},
    zone::{Colliders, Obb, ZoneEntity},
};

pub const VILLAGE_SPAWN: Vec3 = Vec3::new(0.0, 0.0, 7.0);
pub const RUINS_RETURN: Vec3 = Vec3::new(RUINS.x - 5.0, 0.0, RUINS.y + 6.0);

/// 一度作ったら使い回す重いメッシュ。
#[derive(Resource, Default)]
pub struct FieldCache {
    pub terrain: Option<Handle<Mesh>>,
    pub grass: Vec<Handle<Mesh>>,
    pub village: Option<(Handle<Mesh>, Handle<Mesh>)>,
    pub ruins: Option<Handle<Mesh>>,
    pub dungeon: Option<(Handle<Mesh>, Handle<Mesh>)>,
    pub water: Option<Handle<Mesh>>,
    pub mats: Vec<Handle<StandardMaterial>>,
}

#[derive(SystemParam)]
pub struct SpawnCtx<'w> {
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub mats: ResMut<'w, Assets<StandardMaterial>>,
    pub ga: Res<'w, GameAssets>,
    pub cache: ResMut<'w, FieldCache>,
    pub progress: Res<'w, Progress>,
}

/// 水面 (頂点を毎フレーム揺らす)。
#[derive(Component)]
pub struct Water {
    pub base: Vec<[f32; 3]>,
}

const WATER_N: usize = 48;
const WATER_SIZE: f32 = 64.0;

fn water_mesh() -> Mesh {
    let mut pos = Vec::new();
    let mut idx = Vec::new();
    for k in 0..=WATER_N {
        for i in 0..=WATER_N {
            let x = -WATER_SIZE / 2.0 + i as f32 / WATER_N as f32 * WATER_SIZE;
            let z = -WATER_SIZE / 2.0 + k as f32 / WATER_N as f32 * WATER_SIZE;
            pos.push([x, 0.0, z]);
        }
    }
    let w = WATER_N as u32 + 1;
    for k in 0..WATER_N as u32 {
        for i in 0..WATER_N as u32 {
            let a = k * w + i;
            idx.extend_from_slice(&[a, a + w, a + 1, a + 1, a + w, a + w + 1]);
        }
    }
    let n = pos.len();
    Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n])
    .with_inserted_indices(bevy::mesh::Indices::U32(idx))
}

pub fn spawn_field(commands: &mut Commands, ctx: &mut SpawnCtx, col: &mut Colliders) {
    let ga = ctx.ga.clone_handles();

    // 地形
    let terrain_mesh = ctx
        .cache
        .terrain
        .get_or_insert_with(|| ctx.meshes.add(terrain::build_terrain_mesh()))
        .clone();
    commands.spawn((
        Mesh3d(terrain_mesh),
        MeshMaterial3d(ga.vcol.clone()),
        Transform::IDENTITY,
        ZoneEntity,
    ));

    if ctx.cache.grass.is_empty() {
        let chunks = terrain::build_grass_chunks();
        ctx.cache.grass = chunks.into_iter().map(|m| ctx.meshes.add(m)).collect();
    }
    for g in ctx.cache.grass.clone() {
        commands.spawn((
            Mesh3d(g),
            MeshMaterial3d(ga.vcol_double.clone()),
            NotShadowCaster,
            ZoneEntity,
        ));
    }

    // 水面
    let water = ctx
        .cache
        .water
        .get_or_insert_with(|| ctx.meshes.add(water_mesh()))
        .clone();
    let water_mat = ctx.mats.add(StandardMaterial {
        base_color: Color::srgba(0.12, 0.38, 0.52, 0.78),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.06,
        reflectance: 0.9,
        specular_tint: Color::srgb(0.9, 0.95, 1.0),
        ..default()
    });
    let base: Vec<[f32; 3]> = {
        let m = water_mesh();
        match m.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(bevy::mesh::VertexAttributeValues::Float32x3(v)) => v.clone(),
            _ => vec![],
        }
    };
    commands.spawn((
        Mesh3d(water),
        MeshMaterial3d(water_mat),
        Transform::from_xyz(LAKE.x, WATER_Y, LAKE.y),
        NotShadowCaster,
        Water { base },
        ZoneEntity,
    ));

    spawn_village(commands, ctx, col);
    spawn_ruins(commands, ctx, col);
    spawn_nature(commands, ctx, col);
    spawn_field_enemies(commands);
}

impl GameAssets {
    /// 借用を切り離すためのハンドル複製。
    pub fn clone_handles(&self) -> GameAssetsLite {
        GameAssetsLite {
            vcol: self.vcol.clone(),
            vcol_double: self.vcol_double.clone(),
        }
    }
}

pub struct GameAssetsLite {
    pub vcol: Handle<StandardMaterial>,
    pub vcol_double: Handle<StandardMaterial>,
}

fn place(p: Vec2) -> Vec3 {
    Vec3::new(p.x, height(p.x, p.y), p.y)
}

// ---------------------------------------------------------------- 村

struct House {
    pos: Vec2,
    yaw: f32,
    w: f32,
    d: f32,
    roof: Color,
}

fn build_house(lp: &mut LpMesh, glow_lp: &mut LpMesh, h: &House) {
    let base_y = height(h.pos.x, h.pos.y);
    let rot = Quat::from_rotation_y(h.yaw);
    let at = |x: f32, y: f32, z: f32| Vec3::new(h.pos.x, base_y, h.pos.y) + rot * Vec3::new(x, y, z);
    let (hw, hd) = (h.w / 2.0, h.d / 2.0);
    let wall_h = 3.0;
    let stone = Color::srgb(0.5, 0.48, 0.45);
    let plaster = Color::srgb(0.93, 0.88, 0.76);
    let wood = Color::srgb(0.35, 0.22, 0.13);
    lp.cuboid(at(0.0, -0.2, 0.0), Vec3::new(hw + 0.25, 0.7, hd + 0.25), rot, stone);
    lp.cuboid(at(0.0, 0.5 + wall_h / 2.0, 0.0), Vec3::new(hw, wall_h / 2.0, hd), rot, plaster);
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        lp.cuboid(
            at(sx * hw, 0.5 + wall_h / 2.0, sz * hd),
            Vec3::new(0.16, wall_h / 2.0 + 0.02, 0.16),
            rot,
            wood,
        );
    }
    // 梁
    lp.cuboid(at(0.0, 0.5 + wall_h, hd), Vec3::new(hw + 0.1, 0.12, 0.12), rot, wood);
    lp.cuboid(at(0.0, 0.5 + wall_h, -hd), Vec3::new(hw + 0.1, 0.12, 0.12), rot, wood);
    lp.cuboid(at(0.0, 1.9, hd + 0.01), Vec3::new(hw, 0.08, 0.1), rot, wood);
    // 扉
    lp.cuboid(at(0.0, 1.55, hd + 0.05), Vec3::new(0.62, 1.05, 0.08), rot, wood.darker(0.05));
    lp.cuboid(at(0.0, 2.7, hd + 0.08), Vec3::new(0.8, 0.1, 0.12), rot, wood);
    // 窓 (夕方に光る)
    let win = Color::srgb(1.0, 0.8, 0.45);
    for sx in [-1.0, 1.0] {
        let wx = sx * hw * 0.55;
        glow_lp.cuboid(at(wx, 2.4, hd + 0.04), Vec3::new(0.42, 0.38, 0.05), rot, win);
        lp.cuboid(at(wx, 1.95, hd + 0.1), Vec3::new(0.52, 0.06, 0.12), rot, wood);
        glow_lp.cuboid(at(sx * (hw + 0.04), 2.4, 0.0), Vec3::new(0.05, 0.38, 0.42), rot, win);
    }
    // 屋根
    lp.gable_roof(
        at(0.0, 0.5 + wall_h, 0.0),
        Vec3::new(hw + 0.55, 0.0, hd + 0.7),
        2.3,
        rot,
        h.roof,
    );
    // 煙突
    lp.cuboid(at(hw * 0.5, 0.5 + wall_h + 2.0, -hd * 0.35), Vec3::new(0.35, 1.1, 0.35), rot, stone);
}

fn spawn_village(commands: &mut Commands, ctx: &mut SpawnCtx, col: &mut Colliders) {
    let houses = [
        House { pos: Vec2::from_angle(-0.17) * 24.0, yaw: 0.0, w: 8.5, d: 6.0, roof: Color::srgb(0.62, 0.2, 0.16) },
        House { pos: Vec2::from_angle(0.6) * 23.0, yaw: 0.0, w: 6.5, d: 5.5, roof: Color::srgb(0.2, 0.33, 0.55) },
        House { pos: Vec2::from_angle(2.1) * 23.5, yaw: 0.0, w: 7.0, d: 5.5, roof: Color::srgb(0.45, 0.3, 0.2) },
        House { pos: Vec2::from_angle(3.55) * 24.0, yaw: 0.0, w: 6.5, d: 5.0, roof: Color::srgb(0.6, 0.25, 0.15) },
        House { pos: Vec2::from_angle(4.35) * 23.0, yaw: 0.0, w: 7.5, d: 6.0, roof: Color::srgb(0.25, 0.42, 0.3) },
        House { pos: Vec2::from_angle(2.8) * 30.0, yaw: 0.0, w: 6.0, d: 5.0, roof: Color::srgb(0.2, 0.33, 0.55) },
    ];
    let (vmesh, gmesh) = if let Some(v) = &ctx.cache.village {
        v.clone()
    } else {
        let mut lp = LpMesh::default();
        let mut glp = LpMesh::default();
        for h in &houses {
            let mut h2 = House { roof: h.roof, ..*h };
            h2.yaw = (-h.pos.x).atan2(-h.pos.y);
            build_house(&mut lp, &mut glp, &h2);
        }
        build_well(&mut lp);
        build_stall(&mut lp, Vec2::new(10.5, 5.0));
        build_fences(&mut lp);
        let v = (ctx.meshes.add(lp.build()), ctx.meshes.add(glp.build()));
        ctx.cache.village = Some(v.clone());
        v
    };
    for h in &houses {
        let yaw = (-h.pos.x).atan2(-h.pos.y);
        col.boxes.push(Obb {
            center: h.pos,
            half: Vec2::new(h.w / 2.0 + 0.3, h.d / 2.0 + 0.3),
            angle: -yaw,
        });
    }
    col.circles.push((Vec2::ZERO, 1.7));
    col.boxes.push(Obb { center: Vec2::new(10.5, 5.0), half: Vec2::new(2.2, 1.1), angle: 0.0 });

    commands.spawn((Mesh3d(vmesh), MeshMaterial3d(ctx.ga.vcol.clone()), ZoneEntity));
    let win_mat = ctx.mats.add(glow(Color::srgb(1.0, 0.75, 0.4), 3.0));
    commands.spawn((Mesh3d(gmesh), MeshMaterial3d(win_mat), NotShadowCaster, ZoneEntity));

    let ga = &ctx.ga;
    // 焚き火
    let fire = Vec3::new(-5.5, height(-5.5, -3.5), -3.5);
    col.circles.push((fire.xz(), 0.9));
    for i in 0..8 {
        let a = i as f32 / 8.0 * TAU;
        commands.spawn((
            Mesh3d(ga.rocks[i % ga.rocks.len()].clone()),
            MeshMaterial3d(ga.golem_stone.clone()),
            Transform::from_translation(fire + Vec3::new(a.cos() * 0.9, 0.05, a.sin() * 0.9))
                .with_scale(Vec3::splat(0.28)),
            ZoneEntity,
        ));
    }
    for i in 0..3 {
        let a = i as f32 / 3.0 * PI;
        commands.spawn((
            Mesh3d(ga.cylinder.clone()),
            MeshMaterial3d(ga.leather.clone()),
            Transform::from_translation(fire + Vec3::Y * 0.15)
                .with_rotation(Quat::from_rotation_y(a) * Quat::from_rotation_z(FRAC_PI_2 * 0.85))
                .with_scale(Vec3::new(0.22, 1.2, 0.22)),
            ZoneEntity,
        ));
    }
    commands.spawn((
        Mesh3d(ga.cone.clone()),
        MeshMaterial3d(ga.fx_fire.clone()),
        Transform::from_translation(fire + Vec3::Y * 0.55).with_scale(Vec3::new(0.7, 1.0, 0.7)),
        Flicker { base: Vec3::new(0.7, 1.0, 0.7), speed: 11.0, amount: 0.25, light_base: 0.0 },
        NotShadowCaster,
        ZoneEntity,
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(1.0, 0.6, 0.3),
            intensity: 250_000.0,
            range: 16.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(fire + Vec3::Y * 1.2),
        Flicker { base: Vec3::ONE, speed: 9.0, amount: 0.3, light_base: 250_000.0 },
        ZoneEntity,
    ));
    commands.spawn((
        Transform::from_translation(fire + Vec3::Y * 0.6),
        Emitter::new(EmitterKind::Embers, 0.06),
        ZoneEntity,
    ));

    // 街灯
    for (x, z) in [(8.0, -8.0), (-9.0, 8.0), (12.0, 12.0), (-12.0, -10.0)] {
        let p = Vec3::new(x, height(x, z), z);
        col.circles.push((p.xz(), 0.3));
        commands.spawn((
            Mesh3d(ga.cylinder.clone()),
            MeshMaterial3d(ga.leather.clone()),
            Transform::from_translation(p + Vec3::Y * 1.6).with_scale(Vec3::new(0.18, 3.2, 0.18)),
            ZoneEntity,
        ));
        commands.spawn((
            Mesh3d(ga.cube.clone()),
            MeshMaterial3d(ga.fx_gold.clone()),
            Transform::from_translation(p + Vec3::Y * 3.3).with_scale(Vec3::new(0.35, 0.45, 0.35)),
            NotShadowCaster,
            ZoneEntity,
        ));
        commands.spawn((
            Mesh3d(ga.cone.clone()),
            MeshMaterial3d(ga.dark.clone()),
            Transform::from_translation(p + Vec3::Y * 3.7).with_scale(Vec3::new(0.6, 0.35, 0.6)),
            ZoneEntity,
        ));
        commands.spawn((
            PointLight {
                color: Color::srgb(1.0, 0.8, 0.5),
                intensity: 120_000.0,
                range: 12.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_translation(p + Vec3::Y * 3.2),
            ZoneEntity,
        ));
    }

    // 樽と木箱
    let mut rng = Rng::new(55);
    for h in &houses {
        let yaw = (-h.pos.x).atan2(-h.pos.y);
        let rot = Quat::from_rotation_y(yaw);
        for k in 0..2 {
            let local = Vec3::new(h.w / 2.0 + 0.9, 0.0, h.d / 2.0 - 0.6 - k as f32 * 1.1);
            let p2 = h.pos + (rot * local).xz();
            let p = place(p2);
            col.circles.push((p2, 0.5));
            if rng.chance(0.5) {
                commands.spawn((
                    Mesh3d(ga.cylinder.clone()),
                    MeshMaterial3d(ga.leather.clone()),
                    Transform::from_translation(p + Vec3::Y * 0.5).with_scale(Vec3::new(0.9, 1.0, 0.9)),
                    ZoneEntity,
                ));
            } else {
                commands.spawn((
                    Mesh3d(ga.cube.clone()),
                    MeshMaterial3d(ga.leather.clone()),
                    Transform::from_translation(p + Vec3::Y * 0.4)
                        .with_rotation(Quat::from_rotation_y(rng.range(0.0, 1.0)))
                        .with_scale(Vec3::splat(0.8)),
                    ZoneEntity,
                ));
            }
        }
    }

    // セーブクリスタル
    let crystal = Vec3::new(5.0, height(5.0, -9.0), -9.0);
    col.circles.push((crystal.xz(), 0.9));
    commands.spawn((
        Mesh3d(ga.cylinder.clone()),
        MeshMaterial3d(ga.golem_stone.clone()),
        Transform::from_translation(crystal + Vec3::Y * 0.3).with_scale(Vec3::new(1.6, 0.6, 1.6)),
        ZoneEntity,
    ));
    let oct = ctx.meshes.add(crate::util::flat(Sphere::new(1.0).mesh().ico(0).unwrap()));
    commands.spawn((
        Mesh3d(oct),
        MeshMaterial3d(ga.fx_blue.clone()),
        Transform::from_translation(crystal + Vec3::Y * 2.0).with_scale(Vec3::new(0.5, 1.0, 0.5)),
        Spin { axis: Vec3::Y, speed: 0.8, bob: 0.18 },
        NotShadowCaster,
        Interactable { kind: InteractKind::SaveCrystal, radius: 2.6 },
        ZoneEntity,
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(0.4, 0.7, 1.0),
            intensity: 80_000.0,
            range: 9.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(crystal + Vec3::Y * 2.0),
        ZoneEntity,
    ));
    commands.spawn((
        Transform::from_translation(crystal + Vec3::Y * 1.2),
        Emitter::new(EmitterKind::Sparkle, 0.12),
        ZoneEntity,
    ));

    // NPC
    spawn_npc(commands, ga, NpcId::Elder, Vec3::new(-7.5, height(-7.5, -5.5), -5.5), 0.6);
    spawn_npc(commands, ga, NpcId::Merchant, Vec3::new(10.5, height(10.5, 3.3), 3.3), PI);
    spawn_npc(commands, ga, NpcId::Hunter, Vec3::new(-6.0, height(-6.0, 12.0), 12.0), 2.6);
    spawn_npc(commands, ga, NpcId::Child, Vec3::new(3.0, height(3.0, 3.0), 3.0), 0.0);

    // 宝箱
    let chests = [
        (0u8, Vec2::new(-36.0, 25.0)),
        (1, Vec2::new(40.0, 40.0)),
        (2, Vec2::new(-30.0, -60.0)),
        (3, Vec2::new(82.0, -60.0)),
    ];
    for (id, p) in chests {
        spawn_chest(commands, ga, id, place(p), 0.3, ctx.progress.chest_opened(id));
        col.circles.push((p, 0.8));
    }
}

fn build_well(lp: &mut LpMesh) {
    let stone = Color::srgb(0.55, 0.53, 0.5);
    let wood = Color::srgb(0.38, 0.24, 0.14);
    let y = height(0.0, 0.0);
    let n = 10;
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU;
        let p = Vec3::new(a.cos() * 1.25, y + 0.45, a.sin() * 1.25);
        lp.cuboid(p, Vec3::new(0.25, 0.45, 0.42), Quat::from_rotation_y(-a), stone.darker((i % 3) as f32 * 0.03));
    }
    lp.cuboid(Vec3::new(0.0, y + 0.5, 0.0), Vec3::new(1.05, 0.02, 1.05), Quat::IDENTITY, Color::srgb(0.1, 0.2, 0.3));
    for sx in [-1.0, 1.0] {
        lp.cuboid(Vec3::new(sx * 1.2, y + 1.5, 0.0), Vec3::new(0.1, 1.2, 0.1), Quat::IDENTITY, wood);
    }
    lp.cuboid(Vec3::new(0.0, y + 2.4, 0.0), Vec3::new(1.3, 0.07, 0.07), Quat::IDENTITY, wood);
    lp.gable_roof(Vec3::new(0.0, y + 2.6, 0.0), Vec3::new(1.6, 0.0, 1.1), 0.9, Quat::IDENTITY, Color::srgb(0.55, 0.22, 0.15));
}

fn build_stall(lp: &mut LpMesh, p: Vec2) {
    let y = height(p.x, p.y);
    let wood = Color::srgb(0.42, 0.28, 0.16);
    let at = |x: f32, yy: f32, z: f32| Vec3::new(p.x + x, y + yy, p.y + z);
    lp.cuboid(at(0.0, 0.9, 0.0), Vec3::new(2.0, 0.08, 0.9), Quat::IDENTITY, wood);
    lp.cuboid(at(0.0, 0.45, 0.0), Vec3::new(1.9, 0.45, 0.8), Quat::IDENTITY, wood.darker(0.1));
    for (sx, sz) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        lp.cuboid(at(sx * 1.9, 1.4, sz * 0.8), Vec3::new(0.07, 1.4, 0.07), Quat::IDENTITY, wood);
    }
    // 縞模様の日よけ
    let cols = [Color::srgb(0.85, 0.2, 0.2), Color::srgb(0.95, 0.92, 0.85)];
    for i in 0..8 {
        let x0 = -2.2 + i as f32 * 0.55;
        let x1 = x0 + 0.55;
        let c = cols[i % 2];
        lp.quad(at(x0, 2.85, 1.2), at(x1, 2.85, 1.2), at(x1, 3.2, -1.0), at(x0, 3.2, -1.0), c);
        lp.quad(at(x0, 3.2, -1.0), at(x1, 3.2, -1.0), at(x1, 2.85, 1.2), at(x0, 2.85, 1.2), c);
    }
    // 商品
    let goods = [Color::srgb(0.9, 0.3, 0.2), Color::srgb(0.95, 0.8, 0.2), Color::srgb(0.3, 0.7, 0.3), Color::srgb(0.6, 0.3, 0.7)];
    for i in 0..6 {
        lp.cuboid(at(-1.5 + i as f32 * 0.6, 1.1, 0.1 * (i % 2) as f32), Vec3::splat(0.15), Quat::from_rotation_y(i as f32), goods[i % 4]);
    }
}

fn build_fences(lp: &mut LpMesh) {
    let wood = Color::srgb(0.45, 0.32, 0.2);
    // 狩人の家の裏の柵
    let segs = [
        (Vec2::new(-14.0, 16.0), Vec2::new(-4.0, 20.0)),
        (Vec2::new(16.0, 18.0), Vec2::new(24.0, 10.0)),
        (Vec2::new(-26.0, -6.0), Vec2::new(-20.0, -16.0)),
    ];
    for (a, b) in segs {
        let len = a.distance(b);
        let n = (len / 1.6).ceil() as i32;
        for i in 0..=n {
            let p = a.lerp(b, i as f32 / n as f32);
            let y = height(p.x, p.y);
            lp.cuboid(Vec3::new(p.x, y + 0.55, p.y), Vec3::new(0.08, 0.6, 0.08), Quat::IDENTITY, wood);
        }
        for i in 0..n {
            let p0 = a.lerp(b, i as f32 / n as f32);
            let p1 = a.lerp(b, (i + 1) as f32 / n as f32);
            let mid = (p0 + p1) / 2.0;
            let dir = p1 - p0;
            let yaw = -dir.y.atan2(dir.x);
            let y = height(mid.x, mid.y);
            for hh in [0.45, 0.85] {
                lp.cuboid(
                    Vec3::new(mid.x, y + hh, mid.y),
                    Vec3::new(dir.length() / 2.0, 0.05, 0.04),
                    Quat::from_rotation_y(yaw),
                    wood.darker(0.05),
                );
            }
        }
    }
}

// ---------------------------------------------------------------- 遺跡

fn spawn_ruins(commands: &mut Commands, ctx: &mut SpawnCtx, col: &mut Colliders) {
    let c = Vec3::new(RUINS.x, RUINS_H, RUINS.y);
    let pillar_pos: Vec<(Vec2, f32)> = (0..9)
        .map(|i| {
            let a = i as f32 / 9.0 * TAU + 0.2;
            let hgt = [5.5, 2.0, 5.0, 3.2, 5.5, 1.2, 4.6, 5.5, 2.6][i];
            (RUINS + Vec2::from_angle(a) * 10.0, hgt)
        })
        .collect();
    let mesh = if let Some(m) = &ctx.cache.ruins {
        m.clone()
    } else {
        let mut lp = LpMesh::default();
        let mut rng = Rng::new(99);
        // 八角形の石畳
        for ring in 0..3 {
            let r0 = ring as f32 * 3.0;
            let r1 = r0 + 3.0;
            let seg = 8 + ring * 8;
            for i in 0..seg {
                let a0 = i as f32 / seg as f32 * TAU;
                let a1 = (i + 1) as f32 / seg as f32 * TAU;
                let g = rng.range(0.42, 0.56);
                let colr = Color::srgb(g, g * 0.97, g * 0.92);
                let y = c.y + 0.12 + rng.range(-0.04, 0.04);
                let p = |a: f32, r: f32| Vec3::new(c.x + a.cos() * r, y, c.z + a.sin() * r);
                lp.quad(p(a0, r0 + 0.05), p(a0, r1 - 0.05), p(a1, r1 - 0.05), p(a1, r0 + 0.05), colr);
            }
        }
        let stone = Color::srgb(0.6, 0.58, 0.54);
        for (p, hgt) in &pillar_pos {
            let y = height(p.x, p.y).min(c.y);
            lp.cuboid(Vec3::new(p.x, y + 0.25, p.y), Vec3::new(0.9, 0.35, 0.9), Quat::IDENTITY, stone.darker(0.1));
            lp.cuboid(
                Vec3::new(p.x, y + hgt / 2.0, p.y),
                Vec3::new(0.6, hgt / 2.0, 0.6),
                Quat::from_rotation_y(rng.range(-0.1, 0.1)),
                stone,
            );
            if *hgt > 4.5 {
                lp.cuboid(Vec3::new(p.x, y + hgt + 0.2, p.y), Vec3::new(0.85, 0.22, 0.85), Quat::IDENTITY, stone.darker(0.05));
            }
        }
        // 倒れた柱
        for (off, yaw) in [(Vec2::new(4.0, 13.0), 0.4), (Vec2::new(-13.0, -4.0), 1.9)] {
            let p = RUINS + off;
            lp.cuboid(
                Vec3::new(p.x, height(p.x, p.y) + 0.45, p.y),
                Vec3::new(2.4, 0.55, 0.55),
                Quat::from_rotation_y(yaw) * Quat::from_rotation_x(0.2),
                stone.darker(0.12),
            );
        }
        // ポータルの門
        let back = c + Vec3::new(0.0, 0.0, -2.6);
        for sx in [-1.0, 1.0] {
            lp.cuboid(back + Vec3::new(sx * 3.0, 3.0, 0.0), Vec3::new(0.55, 3.0, 0.55), Quat::IDENTITY, stone);
        }
        lp.cuboid(back + Vec3::new(0.0, 6.4, 0.0), Vec3::new(3.9, 0.45, 0.7), Quat::IDENTITY, stone.darker(0.05));
        lp.cuboid(back + Vec3::new(0.0, 0.3, 0.0), Vec3::new(3.6, 0.3, 1.0), Quat::IDENTITY, stone.darker(0.15));
        let m = ctx.meshes.add(lp.build());
        ctx.cache.ruins = Some(m.clone());
        m
    };
    commands.spawn((Mesh3d(mesh), MeshMaterial3d(ctx.ga.vcol.clone()), ZoneEntity));
    for (p, _) in &pillar_pos {
        col.circles.push((*p, 0.95));
    }
    let back = c + Vec3::new(0.0, 0.0, -2.6);
    for sx in [-1.0, 1.0] {
        col.circles.push(((back + Vec3::X * sx * 3.0).xz(), 0.8));
    }

    let ga = &ctx.ga;
    let portal = back + Vec3::new(0.0, 3.1, 0.15);
    commands.spawn((
        Mesh3d(ga.torus.clone()),
        MeshMaterial3d(ga.purple_glow.clone()),
        Transform::from_translation(portal)
            .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
            .with_scale(Vec3::new(2.3, 0.6, 2.3)),
        NotShadowCaster,
        ZoneEntity,
    ));
    let swirl = ctx.mats.add(StandardMaterial {
        base_color: Color::srgba(0.4, 0.1, 0.7, 0.55),
        emissive: LinearRgba::rgb(1.2, 0.3, 2.5),
        alpha_mode: AlphaMode::Add,
        unlit: true,
        cull_mode: None,
        fog_enabled: false,
        ..default()
    });
    commands.spawn((
        Mesh3d(ga.disc.clone()),
        MeshMaterial3d(swirl),
        Transform::from_translation(portal).with_scale(Vec3::splat(2.0)),
        Spin { axis: Vec3::Z, speed: 1.5, bob: 0.0 },
        NotShadowCaster,
        Interactable { kind: InteractKind::Portal, radius: 4.0 },
        ZoneEntity,
    ));
    commands.spawn((
        Transform::from_translation(portal),
        Emitter::new(EmitterKind::Portal, 0.03),
        ZoneEntity,
    ));
    commands.spawn((
        PointLight {
            color: Color::srgb(0.7, 0.35, 1.0),
            intensity: 400_000.0,
            range: 18.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_translation(portal + Vec3::Z * 1.5),
        Flicker { base: Vec3::ONE, speed: 3.0, amount: 0.15, light_base: 400_000.0 },
        ZoneEntity,
    ));
}

// ---------------------------------------------------------------- 自然

fn spawn_nature(commands: &mut Commands, ctx: &mut SpawnCtx, col: &mut Colliders) {
    if ctx.cache.mats.is_empty() {
        let leaf = [
            Color::srgb(0.2, 0.45, 0.2),
            Color::srgb(0.28, 0.55, 0.22),
            Color::srgb(0.38, 0.6, 0.2),
            Color::srgb(0.85, 0.5, 0.15),
            Color::srgb(0.14, 0.34, 0.2),
        ];
        ctx.cache.mats = leaf
            .iter()
            .map(|c| {
                ctx.mats.add(StandardMaterial {
                    base_color: *c,
                    perceptual_roughness: 0.9,
                    ..default()
                })
            })
            .collect();
        let bark = ctx.mats.add(StandardMaterial {
            base_color: Color::srgb(0.36, 0.25, 0.17),
            perceptual_roughness: 0.95,
            ..default()
        });
        ctx.cache.mats.push(bark);
        for g in [0.5, 0.42, 0.58] {
            let m = ctx.mats.add(StandardMaterial {
                base_color: Color::srgb(g, g * 0.97, g * 0.93),
                perceptual_roughness: 0.95,
                ..default()
            });
            ctx.cache.mats.push(m);
        }
    }
    let leaf = &ctx.cache.mats[0..5];
    let bark = ctx.cache.mats[5].clone();
    let rock_mats = &ctx.cache.mats[6..9];
    let ga = &ctx.ga;

    let mut rng = Rng::new(2024);
    let mut trees: Vec<Vec2> = Vec::new();
    for _ in 0..3200 {
        let p = Vec2::new(rng.range(-124.0, 124.0), rng.range(-124.0, 124.0));
        let r = p.length();
        if r > 125.0 || r < 32.0 {
            continue;
        }
        if path_distance(p) < 5.0 || p.distance(LAKE) < 27.0 || p.distance(RUINS) < 17.0 {
            continue;
        }
        let h = height(p.x, p.y);
        if h < WATER_Y + 0.6 || terrain::normal(p.x, p.y).y < 0.78 {
            continue;
        }
        let forest = fbm(p.x * 0.022 + 5.0, p.y * 0.022 - 3.0, 3, 77);
        let dens = if forest > 0.52 { 0.9 } else { 0.06 };
        if !rng.chance(dens) {
            continue;
        }
        if trees.iter().any(|t| t.distance_squared(p) < 10.0) {
            continue;
        }
        if trees.len() > 420 {
            break;
        }
        trees.push(p);
        let pine = r > 92.0 || forest > 0.62 || rng.chance(0.35);
        let s = rng.range(0.8, 1.35);
        let yaw = rng.range(0.0, TAU);
        let root = commands
            .spawn((
                Transform::from_translation(Vec3::new(p.x, h - 0.1, p.y))
                    .with_rotation(Quat::from_rotation_y(yaw))
                    .with_scale(Vec3::splat(s)),
                Visibility::default(),
                ZoneEntity,
            ))
            .id();
        col.circles.push((p, 0.45 * s));
        let trunk_h = if pine { 2.2 } else { 2.8 };
        let trunk = commands
            .spawn((
                Mesh3d(ga.cylinder.clone()),
                MeshMaterial3d(bark.clone()),
                Transform::from_xyz(0.0, trunk_h / 2.0, 0.0).with_scale(Vec3::new(0.42, trunk_h, 0.42)),
            ))
            .id();
        let crown = commands
            .spawn((
                Transform::from_xyz(0.0, trunk_h * 0.8, 0.0),
                Visibility::default(),
                Sway { phase: rng.range(0.0, TAU), amount: 0.025 },
            ))
            .id();
        commands.entity(root).add_children(&[trunk, crown]);
        if pine {
            let m = leaf[if rng.chance(0.6) { 4 } else { 0 }].clone();
            for (i, (y, w, hh)) in [(0.2, 3.0, 2.6), (1.5, 2.3, 2.2), (2.7, 1.5, 1.8)].into_iter().enumerate() {
                let c = commands
                    .spawn((
                        Mesh3d(ga.cone.clone()),
                        MeshMaterial3d(m.clone()),
                        Transform::from_xyz(0.0, y + hh / 2.0, 0.0)
                            .with_rotation(Quat::from_rotation_y(i as f32 * 0.7))
                            .with_scale(Vec3::new(w, hh, w)),
                    ))
                    .id();
                commands.entity(crown).add_child(c);
            }
        } else {
            let m = leaf[if rng.chance(0.12) { 3 } else { 1 + rng.pick(2) }].clone();
            for _ in 0..4 {
                let off = Vec3::new(rng.range(-0.9, 0.9), rng.range(0.8, 2.2), rng.range(-0.9, 0.9));
                let sc = rng.range(1.1, 1.7);
                let c = commands
                    .spawn((
                        Mesh3d(ga.blobs[rng.pick(ga.blobs.len())].clone()),
                        MeshMaterial3d(m.clone()),
                        Transform::from_translation(off)
                            .with_rotation(Quat::from_rotation_y(rng.range(0.0, TAU)))
                            .with_scale(Vec3::splat(sc)),
                    ))
                    .id();
                commands.entity(crown).add_child(c);
            }
        }
    }

    // 岩
    for i in 0..170 {
        let p = Vec2::new(rng.range(-126.0, 126.0), rng.range(-126.0, 126.0));
        let r = p.length();
        if r < 28.0 || path_distance(p) < 3.5 || p.distance(RUINS) < 13.0 {
            continue;
        }
        let big = r > 90.0 || rng.chance(0.25);
        let s = if big { rng.range(1.4, 3.4) } else { rng.range(0.35, 0.9) };
        let h = height(p.x, p.y);
        if h < WATER_Y - 1.0 && !big {
            continue;
        }
        commands.spawn((
            Mesh3d(ga.rocks[i % ga.rocks.len()].clone()),
            MeshMaterial3d(rock_mats[rng.pick(3)].clone()),
            Transform::from_translation(Vec3::new(p.x, h - s * 0.25, p.y))
                .with_rotation(Quat::from_euler(EulerRot::YXZ, rng.range(0.0, TAU), rng.range(-0.2, 0.2), 0.0))
                .with_scale(Vec3::new(s * rng.range(0.9, 1.4), s, s * rng.range(0.9, 1.3))),
            ZoneEntity,
        ));
        if s > 0.8 {
            col.circles.push((p, s * 0.9));
        }
    }

    // 湖の蓮の葉
    let pad_mat = ctx.mats.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.5, 0.2),
        perceptual_roughness: 0.6,
        ..default()
    });
    for _ in 0..26 {
        let p = LAKE + rng.unit_vec2() * rng.range(4.0, 14.0);
        commands.spawn((
            Mesh3d(ga.cylinder.clone()),
            MeshMaterial3d(pad_mat.clone()),
            Transform::from_xyz(p.x, WATER_Y + 0.03, p.y).with_scale(Vec3::new(rng.range(0.6, 1.1), 0.03, rng.range(0.6, 1.1))),
            NotShadowCaster,
            ZoneEntity,
        ));
    }
}

fn spawn_field_enemies(commands: &mut Commands) {
    let mut rng = Rng::new(31337);
    let mut add = |kind: EnemyKind, center: Vec2, spread: f32, n: usize, rng: &mut Rng| {
        let mut placed = 0;
        let mut tries = 0;
        while placed < n && tries < 200 {
            tries += 1;
            let p = center + rng.unit_vec2() * rng.range(0.0, spread);
            if p.length() < 30.0 || height(p.x, p.y) < WATER_Y + 0.3 || p.length() > 112.0 {
                continue;
            }
            commands.spawn((
                SpawnPoint::new(kind, Vec3::new(p.x, height(p.x, p.y), p.y)),
                ZoneEntity,
            ));
            placed += 1;
        }
    };
    // 村の近くのスライム
    add(EnemyKind::Slime, Vec2::new(-20.0, 42.0), 16.0, 5, &mut rng);
    add(EnemyKind::Slime, Vec2::new(34.0, 12.0), 14.0, 5, &mut rng);
    add(EnemyKind::Slime, Vec2::new(-40.0, -20.0), 14.0, 4, &mut rng);
    // 森の狼
    add(EnemyKind::Wolf, Vec2::new(55.0, 40.0), 18.0, 4, &mut rng);
    add(EnemyKind::Wolf, Vec2::new(-65.0, -45.0), 18.0, 4, &mut rng);
    add(EnemyKind::Wolf, Vec2::new(10.0, 80.0), 16.0, 3, &mut rng);
    // 遺跡周辺のスケルトン
    add(EnemyKind::Skeleton, Vec2::new(50.0, -80.0), 12.0, 3, &mut rng);
    add(EnemyKind::Skeleton, Vec2::new(82.0, -48.0), 10.0, 2, &mut rng);
}
