//! ゾーン (フィールド / ダンジョン) の切り替え、地面の高さ、静的な当たり判定。

use bevy::prelude::*;

use crate::{dungeon, field, terrain};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ZoneId {
    #[default]
    Field,
    Dungeon,
}

impl ZoneId {
    pub fn name(self) -> &'static str {
        match self {
            ZoneId::Field => "エルディア平原",
            ZoneId::Dungeon => "黒騎士の古代遺跡",
        }
    }
}

#[derive(Resource, Default)]
pub struct CurrentZone(pub ZoneId);

/// ゾーンを離れると消えるエンティティ。
#[derive(Component)]
pub struct ZoneEntity;

#[derive(Message, Clone, Copy)]
pub struct ZoneRequest {
    pub zone: ZoneId,
    pub spawn: Vec3,
    pub facing: f32,
}

/// ゾーン切り替え完了の通知。
#[derive(Message, Clone, Copy)]
pub struct ZoneEntered(pub ZoneId);

#[derive(Clone, Copy)]
pub struct Obb {
    pub center: Vec2,
    pub half: Vec2,
    pub angle: f32,
}

/// 床/壁のタイルマップ (ダンジョン用)。
#[derive(Clone, Default)]
pub struct TileGrid {
    pub w: i32,
    pub h: i32,
    pub size: f32,
    pub walls: Vec<bool>,
}

impl TileGrid {
    pub fn is_wall(&self, tx: i32, tz: i32) -> bool {
        if tx < 0 || tz < 0 || tx >= self.w || tz >= self.h {
            return true;
        }
        self.walls[(tz * self.w + tx) as usize]
    }
    pub fn tile_of(&self, p: Vec2) -> (i32, i32) {
        ((p.x / self.size).floor() as i32, (p.y / self.size).floor() as i32)
    }
    pub fn wall_at(&self, p: Vec2) -> bool {
        let (x, z) = self.tile_of(p);
        self.is_wall(x, z)
    }
}

#[derive(Resource, Default)]
pub struct Colliders {
    pub circles: Vec<(Vec2, f32)>,
    pub boxes: Vec<Obb>,
    pub grid: Option<TileGrid>,
    /// ボス戦中の封鎖などの一時的な壁
    pub dynamic_boxes: Vec<Obb>,
}

impl Colliders {
    pub fn clear(&mut self) {
        *self = Colliders::default();
    }

    /// 半径 r の円を静的な障害物から押し出す。
    pub fn resolve(&self, mut p: Vec2, r: f32) -> Vec2 {
        for _ in 0..2 {
            for &(c, cr) in &self.circles {
                let d = p - c;
                let min = r + cr;
                let l2 = d.length_squared();
                if l2 < min * min && l2 > 1e-6 {
                    let l = l2.sqrt();
                    p = c + d / l * min;
                }
            }
            for b in self.boxes.iter().chain(self.dynamic_boxes.iter()) {
                p = push_out_obb(p, r, b);
            }
            if let Some(g) = &self.grid {
                let (tx, tz) = g.tile_of(p);
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        let (x, z) = (tx + dx, tz + dz);
                        if g.is_wall(x, z) {
                            let min = Vec2::new(x as f32, z as f32) * g.size;
                            let half = Vec2::splat(g.size * 0.5);
                            p = push_out_obb(
                                p,
                                r,
                                &Obb {
                                    center: min + half,
                                    half,
                                    angle: 0.0,
                                },
                            );
                        }
                    }
                }
            }
        }
        p
    }

    /// 視線 (カメラ用) : from から to へ進み、壁に当たる手前の割合を返す。
    pub fn ray_clear_fraction(&self, from: Vec2, to: Vec2) -> f32 {
        let Some(g) = &self.grid else { return 1.0 };
        let steps = 40;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            if g.wall_at(from.lerp(to, t)) {
                return ((i - 1) as f32 / steps as f32 - 0.04).max(0.05);
            }
        }
        1.0
    }
}

fn push_out_obb(p: Vec2, r: f32, b: &Obb) -> Vec2 {
    let rot = Vec2::from_angle(-b.angle);
    let local = rot.rotate(p - b.center);
    let clamped = local.clamp(-b.half, b.half);
    let d = local - clamped;
    let l2 = d.length_squared();
    if l2 >= r * r {
        return p;
    }
    let pushed = if l2 > 1e-8 {
        clamped + d / l2.sqrt() * r
    } else {
        // 中心が箱の中: 一番近い辺へ押し出す
        let dx = b.half.x - local.x.abs();
        let dz = b.half.y - local.y.abs();
        if dx < dz {
            Vec2::new((b.half.x + r) * local.x.signum(), local.y)
        } else {
            Vec2::new(local.x, (b.half.y + r) * local.y.signum())
        }
    };
    b.center + Vec2::from_angle(b.angle).rotate(pushed)
}

/// 地面の高さ。ダンジョンは平らな床。
pub fn ground_height(zone: ZoneId, p: Vec2) -> f32 {
    match zone {
        ZoneId::Field => terrain::height(p.x, p.y).max(terrain::WATER_Y - 0.9),
        ZoneId::Dungeon => 0.0,
    }
}

/// 歩いて入れない場所 (深い水・世界の外)。
pub fn blocked(zone: ZoneId, p: Vec2) -> bool {
    match zone {
        ZoneId::Field => {
            p.length() > terrain::WORLD_RADIUS || terrain::height(p.x, p.y) < terrain::WATER_Y - 0.8
        }
        ZoneId::Dungeon => false,
    }
}

pub struct ZonePlugin;

impl Plugin for ZonePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentZone>()
            .init_resource::<field::FieldCache>()
            .init_resource::<Colliders>()
            .add_message::<ZoneRequest>()
            .add_message::<ZoneEntered>()
            .add_systems(Startup, initial_zone)
            .add_systems(PostUpdate, apply_zone_request);
    }
}

fn initial_zone(mut w: MessageWriter<ZoneRequest>) {
    w.write(ZoneRequest {
        zone: ZoneId::Field,
        spawn: field::VILLAGE_SPAWN,
        facing: 0.0,
    });
}

#[allow(clippy::too_many_arguments)]
fn apply_zone_request(
    mut commands: Commands,
    mut reqs: MessageReader<ZoneRequest>,
    mut entered: MessageWriter<ZoneEntered>,
    zone_q: Query<Entity, With<ZoneEntity>>,
    mut current: ResMut<CurrentZone>,
    mut colliders: ResMut<Colliders>,
    mut player: Query<&mut Transform, With<crate::player::Player>>,
    mut ctx: field::SpawnCtx,
) {
    let Some(req) = reqs.read().last().copied() else {
        return;
    };
    for e in &zone_q {
        commands.entity(e).despawn();
    }
    colliders.clear();
    current.0 = req.zone;
    match req.zone {
        ZoneId::Field => field::spawn_field(&mut commands, &mut ctx, &mut colliders),
        ZoneId::Dungeon => dungeon::spawn_dungeon(&mut commands, &mut ctx, &mut colliders),
    }
    for mut t in &mut player {
        t.translation = req.spawn;
        t.translation.y = ground_height(req.zone, req.spawn.xz());
        t.rotation = Quat::from_rotation_y(req.facing);
    }
    entered.write(ZoneEntered(req.zone));
}
