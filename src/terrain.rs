//! フィールドの地形 (高さ関数・メッシュ・草)。

use bevy::prelude::*;

use crate::util::{LpMesh, Rng, fbm, smoothstep};

pub const HALF: f32 = 150.0;
pub const CELL: f32 = 2.0;
pub const WATER_Y: f32 = -0.45;
pub const LAKE: Vec2 = Vec2::new(-58.0, 34.0);
pub const RUINS: Vec2 = Vec2::new(66.0, -70.0);
pub const RUINS_H: f32 = 3.2;
pub const WORLD_RADIUS: f32 = 128.0;

/// 村 → 遺跡、村 → 湖 の道 (折れ線)。
pub const PATHS: &[&[Vec2]] = &[
    &[
        Vec2::new(0.0, 0.0),
        Vec2::new(14.0, -22.0),
        Vec2::new(38.0, -34.0),
        Vec2::new(52.0, -58.0),
        RUINS,
    ],
    &[
        Vec2::new(0.0, 0.0),
        Vec2::new(-22.0, 10.0),
        Vec2::new(-40.0, 26.0),
    ],
    &[Vec2::new(0.0, 0.0), Vec2::new(6.0, 30.0), Vec2::new(-4.0, 60.0)],
];

pub fn path_distance(p: Vec2) -> f32 {
    let mut best = f32::MAX;
    for path in PATHS {
        for w in path.windows(2) {
            let (a, b) = (w[0], w[1]);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
            best = best.min((a + ab * t).distance(p));
        }
    }
    best
}

/// 地形の高さ。地形メッシュと当たり判定の両方がこれを使う。
pub fn height(x: f32, z: f32) -> f32 {
    let p = Vec2::new(x, z);
    let r = p.length();
    let mut h = (fbm(x * 0.016 + 37.0, z * 0.016 - 11.0, 5, 1) - 0.45) * 16.0;
    h += (fbm(x * 0.07, z * 0.07, 3, 9) - 0.5) * 1.4;

    // 外周の山並み
    let m = smoothstep(92.0, 138.0, r);
    h += m * (16.0 + fbm(x * 0.028, z * 0.028, 4, 3) * 34.0);

    // 道はなだらかに
    let pd = path_distance(p);
    let flat_path = smoothstep(2.0, 9.0, pd);
    h *= (0.55 + 0.45 * flat_path);

    // 村の広場
    let v = smoothstep(20.0, 40.0, r);
    h = 0.5 + (h - 0.5) * v;

    // 湖
    let dl = p.distance(LAKE);
    let lake = 1.0 - smoothstep(8.0, 25.0, dl);
    h = h * (1.0 - lake) + (-3.2) * lake;

    // 遺跡の台地
    let dr = p.distance(RUINS);
    let ruins = smoothstep(12.0, 24.0, dr);
    h = RUINS_H + (h - RUINS_H) * ruins;
    h
}

pub fn normal(x: f32, z: f32) -> Vec3 {
    let e = 0.5;
    let dx = height(x + e, z) - height(x - e, z);
    let dz = height(x, z + e) - height(x, z - e);
    Vec3::new(-dx, 2.0 * e, -dz).normalize()
}

/// 草が生える場所か (0..1 の密度)。
pub fn grass_density(x: f32, z: f32) -> f32 {
    let p = Vec2::new(x, z);
    let h = height(x, z);
    if h < WATER_Y + 0.4 || p.length() > 118.0 {
        return 0.0;
    }
    let n = normal(x, z);
    if n.y < 0.82 {
        return 0.0;
    }
    let pd = path_distance(p);
    let plaza = p.length();
    let d = smoothstep(2.4, 4.0, pd) * smoothstep(13.0, 17.0, plaza) * smoothstep(9.0, 13.0, p.distance(RUINS));
    d * (0.35 + 0.65 * fbm(x * 0.05, z * 0.05, 3, 21))
}

fn terrain_color(center: Vec3, n: Vec3, rng: &mut Rng) -> Color {
    let (x, z, h) = (center.x, center.z, center.y);
    let p = Vec2::new(x, z);
    let j = rng.range(-0.035, 0.035);
    let pd = path_distance(p);
    let g = fbm(x * 0.04, z * 0.04, 3, 5);
    let lush = Color::srgb(0.28 + j, 0.52 + j, 0.2);
    let dry = Color::srgb(0.47 + j, 0.6 + j, 0.27);
    let dark = Color::srgb(0.2 + j, 0.4 + j, 0.17);
    let mut c = if g < 0.42 {
        dark.mix(&lush, (g - 0.3).max(0.0) / 0.12)
    } else {
        lush.mix(&dry, ((g - 0.42) / 0.2).min(1.0))
    };
    let dirt = Color::srgb(0.56 + j, 0.44 + j, 0.3);
    let sand = Color::srgb(0.82 + j, 0.75 + j, 0.55);
    let rock = Color::srgb(0.47 + j, 0.45 + j, 0.42 + j);
    let snow = Color::srgb(0.93 + j * 0.5, 0.95 + j * 0.5, 0.98);
    if pd < 2.6 + fbm(x * 0.3, z * 0.3, 2, 8) * 1.2 {
        c = dirt;
    }
    let plaza = p.length();
    if plaza < 13.5 + fbm(x * 0.2, z * 0.2, 2, 4) * 3.0 {
        c = Color::srgb(0.62 + j, 0.57 + j, 0.48 + j);
    }
    if p.distance(RUINS) < 12.0 {
        c = Color::srgb(0.52 + j, 0.5 + j, 0.47 + j);
    }
    if h < WATER_Y + 0.6 {
        c = sand;
    }
    if h < WATER_Y - 0.3 {
        c = Color::srgb(0.42 + j, 0.4, 0.3);
    }
    if n.y < 0.72 {
        c = rock;
    }
    if h > 30.0 + fbm(x * 0.1, z * 0.1, 2, 2) * 8.0 && n.y > 0.6 {
        c = snow;
    }
    c
}

pub fn build_terrain_mesh() -> Mesh {
    let n = (HALF * 2.0 / CELL) as i32;
    let mut rng = Rng::new(7);
    let mut lp = LpMesh::default();
    let v = |i: i32, k: i32| {
        let x = -HALF + i as f32 * CELL;
        let z = -HALF + k as f32 * CELL;
        Vec3::new(x, height(x, z), z)
    };
    for i in 0..n {
        for k in 0..n {
            let a = v(i, k);
            let b = v(i + 1, k);
            let c = v(i + 1, k + 1);
            let d = v(i, k + 1);
            // 対角線を交互に切ってローポリらしい模様にする
            let tris = if (i + k) % 2 == 0 {
                [[a, d, c], [a, c, b]]
            } else {
                [[a, d, b], [b, d, c]]
            };
            for [p, q, r] in tris {
                let nrm = (q - p).cross(r - p).normalize_or_zero();
                let col = terrain_color((p + q + r) / 3.0, nrm, &mut rng);
                lp.tri_n(p, q, r, nrm, col);
            }
        }
    }
    lp.build()
}

/// 草と花のチャンクメッシュ。
pub fn build_grass_chunks() -> Vec<Mesh> {
    let chunks = 8;
    let span = 236.0 / chunks as f32;
    let mut out = Vec::new();
    let mut rng = Rng::new(1234);
    for cx in 0..chunks {
        for cz in 0..chunks {
            let mut lp = LpMesh::default();
            let x0 = -118.0 + cx as f32 * span;
            let z0 = -118.0 + cz as f32 * span;
            let tries = 1400;
            for _ in 0..tries {
                let x = x0 + rng.f() * span;
                let z = z0 + rng.f() * span;
                let d = grass_density(x, z);
                if d <= 0.0 || rng.f() > d {
                    continue;
                }
                let base = Vec3::new(x, height(x, z) - 0.05, z);
                let tint = rng.range(-0.05, 0.05);
                let root = Color::srgb(0.16 + tint, 0.34 + tint, 0.12);
                let tip = Color::srgb(0.52 + tint, 0.72 + tint, 0.3);
                let blades = 4;
                for b in 0..blades {
                    let a = rng.range(0.0, std::f32::consts::TAU);
                    let dir = Vec3::new(a.cos(), 0.0, a.sin());
                    let side = Vec3::new(-dir.z, 0.0, dir.x);
                    let hgt = rng.range(0.35, 0.75);
                    let w = 0.07;
                    let off = dir * rng.range(0.0, 0.18) + Vec3::Y * 0.0;
                    let lean = dir * rng.range(0.1, 0.3) + side * rng.range(-0.1, 0.1);
                    let p0 = base + off - side * w;
                    let p1 = base + off + side * w;
                    let p2 = base + off + lean + Vec3::Y * hgt;
                    let nrm = Vec3::new(dir.x * 0.3, 1.0, dir.z * 0.3).normalize();
                    let _ = b;
                    push_tri_vc(&mut lp, [p0, p1, p2], nrm, [root, root, tip]);
                }
                // 花
                if rng.chance(0.07) {
                    let cols = [
                        Color::srgb(1.0, 0.95, 0.9),
                        Color::srgb(1.0, 0.85, 0.2),
                        Color::srgb(0.95, 0.45, 0.65),
                        Color::srgb(0.55, 0.6, 1.0),
                    ];
                    let col = cols[rng.pick(cols.len())];
                    let top = base + Vec3::Y * rng.range(0.35, 0.55);
                    let r = 0.11;
                    for k in 0..5 {
                        let a0 = k as f32 / 5.0 * std::f32::consts::TAU;
                        let a1 = (k as f32 + 0.5) / 5.0 * std::f32::consts::TAU;
                        let q0 = top + Vec3::new(a0.cos(), 0.03, a0.sin()) * r;
                        let q1 = top + Vec3::new(a1.cos(), 0.0, a1.sin()) * r * 0.4;
                        push_tri_vc(&mut lp, [top, q0, q1], Vec3::Y, [Color::srgb(1.0, 0.9, 0.3), col, col]);
                        push_tri_vc(&mut lp, [top, q1, q0], Vec3::Y, [Color::srgb(1.0, 0.9, 0.3), col, col]);
                    }
                }
            }
            if !lp.is_empty() {
                out.push(lp.build());
            }
        }
    }
    out
}

fn push_tri_vc(lp: &mut LpMesh, p: [Vec3; 3], n: Vec3, c: [Color; 3]) {
    for i in 0..3 {
        lp.pos.push(p[i].to_array());
        lp.nrm.push(n.to_array());
        lp.col.push(c[i].to_linear().to_f32_array());
    }
}
