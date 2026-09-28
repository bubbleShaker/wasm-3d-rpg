//! 乱数・ノイズ・ローポリメッシュ生成などの汎用ユーティリティ。

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

/// 決定的な軽量乱数 (xorshift64*)。
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// [0, 1)
    pub fn f(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }
    pub fn pick(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
    pub fn unit_vec2(&mut self) -> Vec2 {
        let a = self.range(0.0, std::f32::consts::TAU);
        Vec2::new(a.cos(), a.sin())
    }
    pub fn unit_vec3(&mut self) -> Vec3 {
        let z = self.range(-1.0, 1.0);
        let a = self.range(0.0, std::f32::consts::TAU);
        let r = (1.0 - z * z).sqrt();
        Vec3::new(r * a.cos(), z, r * a.sin())
    }
}

/// ゲーム全体で共有する乱数 (演出・ダメージ揺らぎ用)。
#[derive(Resource)]
pub struct GameRng(pub Rng);

impl Default for GameRng {
    fn default() -> Self {
        Self(Rng::new(0xC0FFEE))
    }
}

fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393)
        ^ (y as u32).wrapping_mul(668_265_263)
        ^ seed.wrapping_mul(2_246_822_519);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 0x00FF_FFFF as f32
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// 値ノイズ (0..1)。
pub fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let tx = smooth(x - xi as f32);
    let ty = smooth(y - yi as f32);
    let a = hash2(xi, yi, seed);
    let b = hash2(xi + 1, yi, seed);
    let c = hash2(xi, yi + 1, seed);
    let d = hash2(xi + 1, yi + 1, seed);
    let ab = a + (b - a) * tx;
    let cd = c + (d - c) * tx;
    ab + (cd - ab) * ty
}

/// フラクタルノイズ (おおよそ 0..1)。
pub fn fbm(x: f32, y: f32, octaves: u32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut norm = 0.0;
    for i in 0..octaves {
        sum += value_noise(x * freq, y * freq, seed + i * 17) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.03;
    }
    sum / norm
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 指数減衰による補間係数 (フレームレート非依存)。
pub fn damp(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

pub fn lerp_angle(a: f32, b: f32, t: f32) -> f32 {
    let mut d = (b - a) % std::f32::consts::TAU;
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    } else if d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    a + d * t
}

/// 頂点カラー付きのフラットシェーディングメッシュを組み立てるビルダー。
#[derive(Default)]
pub struct LpMesh {
    pub pos: Vec<[f32; 3]>,
    pub nrm: Vec<[f32; 3]>,
    pub col: Vec<[f32; 4]>,
}

impl LpMesh {
    pub fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, color: Color) {
        let n = (b - a).cross(c - a).normalize_or_zero();
        self.tri_n(a, b, c, n, color);
    }

    pub fn tri_n(&mut self, a: Vec3, b: Vec3, c: Vec3, n: Vec3, color: Color) {
        let col = color.to_linear().to_f32_array();
        for p in [a, b, c] {
            self.pos.push(p.to_array());
            self.nrm.push(n.to_array());
            self.col.push(col);
        }
    }

    pub fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, color: Color) {
        self.tri(a, b, c, color);
        self.tri(a, c, d, color);
    }

    /// 回転付きの直方体。
    pub fn cuboid(&mut self, center: Vec3, half: Vec3, rot: Quat, color: Color) {
        let p = |x: f32, y: f32, z: f32| center + rot * (Vec3::new(x, y, z) * half);
        let v = [
            p(-1., -1., -1.),
            p(1., -1., -1.),
            p(1., 1., -1.),
            p(-1., 1., -1.),
            p(-1., -1., 1.),
            p(1., -1., 1.),
            p(1., 1., 1.),
            p(-1., 1., 1.),
        ];
        let faces = [
            [4, 5, 6, 7], // +z
            [1, 0, 3, 2], // -z
            [5, 1, 2, 6], // +x
            [0, 4, 7, 3], // -x
            [7, 6, 2, 3], // +y
            [0, 1, 5, 4], // -y
        ];
        for f in faces {
            self.quad(v[f[0]], v[f[1]], v[f[2]], v[f[3]], color);
        }
    }

    /// 三角屋根 (切妻) : 底面の中心・半幅・高さ。棟は x 方向に伸びる。
    pub fn gable_roof(&mut self, base: Vec3, half: Vec3, height: f32, rot: Quat, color: Color) {
        let p = |x: f32, y: f32, z: f32| base + rot * Vec3::new(x, y, z);
        let (hx, hz) = (half.x, half.z);
        let a = p(-hx, 0.0, -hz);
        let b = p(hx, 0.0, -hz);
        let c = p(hx, 0.0, hz);
        let d = p(-hx, 0.0, hz);
        let r0 = p(-hx, height, 0.0);
        let r1 = p(hx, height, 0.0);
        self.quad(d, c, r1, r0, color);
        self.quad(b, a, r0, r1, color);
        self.tri(a, d, r0, color.darker(0.08));
        self.tri(c, b, r1, color.darker(0.08));
        self.quad(a, b, c, d, color.darker(0.2));
    }

    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }

    pub fn build(self) -> Mesh {
        let n = self.pos.len() as u32;
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
        .with_inserted_indices(Indices::U32((0..n).collect()))
    }
}

/// 頂点を少しずつ揺らしたフラットシェーディングの岩・葉っぱメッシュ。
pub fn jittered_ico(subdiv: u32, jitter: f32, squash: Vec3, seed: u64) -> Mesh {
    let mut rng = Rng::new(seed);
    let mut mesh = Sphere::new(1.0).mesh().ico(subdiv).unwrap();
    if let Some(bevy::mesh::VertexAttributeValues::Float32x3(ps)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    {
        for p in ps.iter_mut() {
            let k = 1.0 + rng.range(-jitter, jitter);
            *p = [p[0] * k * squash.x, p[1] * k * squash.y, p[2] * k * squash.z];
        }
    }
    mesh.with_duplicated_vertices().with_computed_flat_normals()
}

/// フラットシェーディング化 (プリミティブメッシュ用)。
pub fn flat(mesh: impl Into<Mesh>) -> Mesh {
    mesh.into()
        .with_duplicated_vertices()
        .with_computed_flat_normals()
}
