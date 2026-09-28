//! パーティクルなどの視覚効果。

use std::f32::consts::{PI, TAU};

use bevy::{light::NotShadowCaster, prelude::*};

use crate::{
    assets::GameAssets,
    util::{GameRng, Rng},
    zone::ZoneEntity,
};

#[derive(Component)]
pub struct Particle {
    pub vel: Vec3,
    pub life: f32,
    pub max: f32,
    pub gravity: f32,
    pub drag: f32,
    pub size: f32,
    pub end_size: f32,
}

/// 一定時間で消える (拡大縮小付き) エンティティ。
#[derive(Component)]
pub struct Timed {
    pub life: f32,
    pub max: f32,
    pub grow: Option<(Vec3, Vec3)>,
}

impl Timed {
    pub fn new(life: f32) -> Self {
        Self { life, max: life, grow: None }
    }
    pub fn grow(mut self, from: Vec3, to: Vec3) -> Self {
        self.grow = Some((from, to));
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EmitterKind {
    Embers,
    Sparkle,
    Portal,
}

#[derive(Component)]
pub struct Emitter {
    pub kind: EmitterKind,
    pub interval: f32,
    pub acc: f32,
}

impl Emitter {
    pub fn new(kind: EmitterKind, interval: f32) -> Self {
        Self { kind, interval, acc: 0.0 }
    }
}

/// 炎・光源のゆらぎ。
#[derive(Component)]
pub struct Flicker {
    pub base: Vec3,
    pub speed: f32,
    pub amount: f32,
    pub light_base: f32,
}

#[derive(Component)]
pub struct Spin {
    pub axis: Vec3,
    pub speed: f32,
    pub bob: f32,
}

/// 木の揺れ。
#[derive(Component)]
pub struct Sway {
    pub phase: f32,
    pub amount: f32,
}

#[derive(Clone, Copy)]
pub enum Burst {
    Hit,
    Crit,
    Blood,
    Death,
    Step,
    Land,
    LevelUp,
    Heal,
    Shock(f32),
    Magic,
    Trail,
    Chest,
}

pub struct VfxPlugin;

impl Plugin for VfxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (particles, timed, emitters, flicker, spin, sway));
    }
}

fn spawn_particle(
    commands: &mut Commands,
    mesh: &Handle<Mesh>,
    mat: &Handle<StandardMaterial>,
    pos: Vec3,
    p: Particle,
) {
    commands.spawn((
        Mesh3d(mesh.clone()),
        MeshMaterial3d(mat.clone()),
        Transform::from_translation(pos).with_scale(Vec3::splat(p.size)),
        p,
        NotShadowCaster,
        ZoneEntity,
    ));
}

pub fn burst(commands: &mut Commands, ga: &GameAssets, rng: &mut Rng, kind: Burst, pos: Vec3) {
    let m = &ga.sphere_lp;
    match kind {
        Burst::Hit | Burst::Crit => {
            let n = if matches!(kind, Burst::Crit) { 18 } else { 10 };
            for _ in 0..n {
                let v = rng.unit_vec3() * rng.range(4.0, 10.0);
                spawn_particle(commands, m, &ga.fx_spark, pos, Particle { vel: v, life: 0.0, max: rng.range(0.15, 0.35), gravity: 12.0, drag: 4.0, size: rng.range(0.05, 0.1), end_size: 0.0 });
            }
            let mat = if matches!(kind, Burst::Crit) { &ga.fx_gold } else { &ga.fx_spark };
            commands.spawn((
                Mesh3d(ga.sphere.clone()),
                MeshMaterial3d(mat.clone()),
                Transform::from_translation(pos).with_scale(Vec3::splat(0.1)),
                Timed::new(0.12).grow(Vec3::splat(0.2), Vec3::splat(if matches!(kind, Burst::Crit) { 0.9 } else { 0.55 })),
                NotShadowCaster,
                ZoneEntity,
            ));
        }
        Burst::Blood => {
            for _ in 0..10 {
                let v = rng.unit_vec3() * rng.range(3.0, 6.0) + Vec3::Y * 2.0;
                spawn_particle(commands, m, &ga.fx_blood, pos, Particle { vel: v, life: 0.0, max: rng.range(0.25, 0.45), gravity: 14.0, drag: 2.0, size: rng.range(0.05, 0.09), end_size: 0.0 });
            }
        }
        Burst::Death => {
            for _ in 0..26 {
                let v = rng.unit_vec3() * rng.range(1.0, 4.0) + Vec3::Y * rng.range(1.0, 4.0);
                spawn_particle(commands, m, &ga.fx_purple, pos + rng.unit_vec3() * 0.4, Particle { vel: v, life: 0.0, max: rng.range(0.5, 1.1), gravity: -1.5, drag: 1.5, size: rng.range(0.08, 0.18), end_size: 0.0 });
            }
        }
        Burst::Step => {
            for _ in 0..3 {
                let v = Vec3::new(rng.range(-1.0, 1.0), rng.range(0.4, 1.2), rng.range(-1.0, 1.0));
                spawn_particle(commands, m, &ga.fx_dust, pos, Particle { vel: v, life: 0.0, max: rng.range(0.35, 0.6), gravity: -0.5, drag: 3.0, size: rng.range(0.08, 0.14), end_size: 0.3 });
            }
        }
        Burst::Land => {
            for i in 0..12 {
                let a = i as f32 / 12.0 * TAU;
                let v = Vec3::new(a.cos() * 3.0, 0.5, a.sin() * 3.0);
                spawn_particle(commands, m, &ga.fx_dust, pos, Particle { vel: v, life: 0.0, max: 0.5, gravity: 0.0, drag: 4.0, size: 0.12, end_size: 0.35 });
            }
        }
        Burst::LevelUp => {
            for i in 0..40 {
                let a = i as f32 / 40.0 * TAU * 3.0;
                let r = 1.0;
                let p = pos + Vec3::new(a.cos() * r, i as f32 * 0.04, a.sin() * r);
                let v = Vec3::new(-a.sin(), 3.5, a.cos()) * 1.2;
                spawn_particle(commands, m, &ga.fx_gold, p, Particle { vel: v, life: 0.0, max: rng.range(0.8, 1.4), gravity: -1.0, drag: 1.0, size: 0.09, end_size: 0.0 });
            }
            commands.spawn((
                Mesh3d(ga.ring.clone()),
                MeshMaterial3d(ga.fx_gold.clone()),
                Transform::from_translation(pos + Vec3::Y * 0.1).with_rotation(Quat::from_rotation_x(-PI / 2.0)),
                Timed::new(0.8).grow(Vec3::splat(0.3), Vec3::splat(4.0)),
                NotShadowCaster,
                ZoneEntity,
            ));
        }
        Burst::Heal => {
            for _ in 0..24 {
                let o = rng.unit_vec2() * rng.range(0.2, 0.8);
                let p = pos + Vec3::new(o.x, rng.range(-0.8, 0.6), o.y);
                spawn_particle(commands, m, &ga.fx_heal, p, Particle { vel: Vec3::Y * rng.range(1.0, 2.5), life: 0.0, max: rng.range(0.6, 1.0), gravity: 0.0, drag: 0.5, size: 0.07, end_size: 0.0 });
            }
        }
        Burst::Shock(r) => {
            commands.spawn((
                Mesh3d(ga.ring.clone()),
                MeshMaterial3d(ga.fx_shock.clone()),
                Transform::from_translation(pos).with_rotation(Quat::from_rotation_x(-PI / 2.0)),
                Timed::new(0.45).grow(Vec3::splat(0.5), Vec3::splat(r * 1.15)),
                NotShadowCaster,
                ZoneEntity,
            ));
            for _ in 0..30 {
                let d = rng.unit_vec2();
                let v = Vec3::new(d.x * rng.range(5.0, 11.0), rng.range(2.0, 6.0), d.y * rng.range(5.0, 11.0));
                spawn_particle(commands, m, &ga.fx_dust, pos, Particle { vel: v, life: 0.0, max: rng.range(0.4, 0.8), gravity: 12.0, drag: 2.0, size: rng.range(0.12, 0.25), end_size: 0.05 });
            }
        }
        Burst::Magic => {
            for _ in 0..14 {
                let v = rng.unit_vec3() * rng.range(2.0, 6.0);
                spawn_particle(commands, m, &ga.fx_purple, pos, Particle { vel: v, life: 0.0, max: rng.range(0.25, 0.5), gravity: 0.0, drag: 3.0, size: 0.1, end_size: 0.0 });
            }
        }
        Burst::Trail => {
            spawn_particle(commands, m, &ga.fx_purple, pos + rng.unit_vec3() * 0.2, Particle { vel: Vec3::ZERO, life: 0.0, max: 0.35, gravity: 0.0, drag: 0.0, size: 0.18, end_size: 0.0 });
        }
        Burst::Chest => {
            for _ in 0..30 {
                let v = rng.unit_vec3() * rng.range(1.0, 3.0) + Vec3::Y * 4.0;
                spawn_particle(commands, m, &ga.fx_gold, pos, Particle { vel: v, life: 0.0, max: rng.range(0.6, 1.2), gravity: 5.0, drag: 1.0, size: 0.08, end_size: 0.0 });
            }
        }
    }
}

/// 剣の残像 (刃の位置に細長い板を置いて短時間で消す)。
pub fn spawn_trail(commands: &mut Commands, ga: &GameAssets, base: Vec3, tip: Vec3, prev_tip: Vec3) {
    let blade = tip - base;
    let len = blade.length();
    if len < 0.01 {
        return;
    }
    let y = blade / len;
    let motion = tip - prev_tip;
    let mut x = motion - y * motion.dot(y);
    let ml = x.length();
    if ml < 1e-4 {
        return;
    }
    x /= ml;
    let z = x.cross(y);
    let rot = Quat::from_mat3(&Mat3::from_cols(x, y, z));
    let width = (ml * 1.1).clamp(0.05, 1.2);
    let s = Vec3::new(width, len, 0.02);
    commands.spawn((
        Mesh3d(ga.cube.clone()),
        MeshMaterial3d(ga.fx_trail.clone()),
        Transform {
            translation: (base + tip) / 2.0 - x * width * 0.5,
            rotation: rot,
            scale: s,
        },
        Timed::new(0.14).grow(s, Vec3::new(width * 0.6, len * 0.9, 0.0)),
        NotShadowCaster,
        ZoneEntity,
    ));
}

fn particles(time: Res<Time>, mut commands: Commands, mut q: Query<(Entity, &mut Transform, &mut Particle)>) {
    let dt = time.delta_secs();
    for (e, mut tf, mut p) in &mut q {
        p.life += dt;
        if p.life >= p.max {
            commands.entity(e).despawn();
            continue;
        }
        let g = p.gravity;
        p.vel.y -= g * dt;
        let drag = p.drag;
        p.vel *= 1.0 / (1.0 + drag * dt);
        tf.translation += p.vel * dt;
        let u = p.life / p.max;
        tf.scale = Vec3::splat(p.size + (p.end_size - p.size) * u);
    }
}

fn timed(time: Res<Time>, mut commands: Commands, mut q: Query<(Entity, &mut Transform, &mut Timed)>) {
    let dt = time.delta_secs();
    for (e, mut tf, mut t) in &mut q {
        t.life -= dt;
        if t.life <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        if let Some((a, b)) = t.grow {
            let u = 1.0 - t.life / t.max;
            tf.scale = a.lerp(b, u);
        }
    }
}

fn emitters(
    time: Res<Time>,
    mut commands: Commands,
    ga: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    mut q: Query<(&GlobalTransform, &mut Emitter)>,
) {
    let dt = time.delta_secs();
    for (gt, mut em) in &mut q {
        em.acc += dt;
        while em.acc >= em.interval {
            em.acc -= em.interval;
            let pos = gt.translation();
            let r = &mut rng.0;
            match em.kind {
                EmitterKind::Embers => {
                    let p = pos + Vec3::new(r.range(-0.2, 0.2), 0.0, r.range(-0.2, 0.2));
                    let v = Vec3::new(r.range(-0.3, 0.3), r.range(1.0, 2.2), r.range(-0.3, 0.3));
                    spawn_particle(&mut commands, &ga.sphere_lp, &ga.fx_fire, p, Particle { vel: v, life: 0.0, max: r.range(0.6, 1.4), gravity: -0.3, drag: 0.5, size: r.range(0.03, 0.06), end_size: 0.0 });
                }
                EmitterKind::Sparkle => {
                    let o = r.unit_vec3() * r.range(0.3, 1.2);
                    spawn_particle(&mut commands, &ga.sphere_lp, &ga.fx_blue, pos + o, Particle { vel: Vec3::Y * r.range(0.3, 0.9), life: 0.0, max: r.range(0.8, 1.6), gravity: 0.0, drag: 0.2, size: 0.05, end_size: 0.0 });
                }
                EmitterKind::Portal => {
                    let a = r.range(0.0, TAU);
                    let rad = r.range(1.6, 2.4);
                    let o = Vec3::new(a.cos() * rad, a.sin() * rad, 0.0);
                    let v = -o * 0.6 + Vec3::Z * r.range(0.2, 0.8);
                    spawn_particle(&mut commands, &ga.sphere_lp, &ga.fx_purple, pos + o, Particle { vel: v, life: 0.0, max: r.range(0.8, 1.4), gravity: 0.0, drag: 0.0, size: 0.07, end_size: 0.0 });
                }
            }
        }
    }
}

fn flicker(time: Res<Time>, mut q: Query<(&mut Transform, &Flicker, Option<&mut PointLight>)>) {
    let t = time.elapsed_secs();
    for (mut tf, f, light) in &mut q {
        let n = (t * f.speed).sin() * 0.5 + (t * f.speed * 2.3 + 1.7).sin() * 0.3 + (t * f.speed * 0.37).sin() * 0.2;
        if let Some(mut l) = light {
            l.intensity = f.light_base * (1.0 + n * f.amount);
        } else {
            tf.scale = f.base * Vec3::new(1.0 - n * f.amount * 0.3, 1.0 + n * f.amount, 1.0 - n * f.amount * 0.3);
        }
    }
}

fn spin(time: Res<Time>, mut q: Query<(&mut Transform, &Spin)>, mut base: Local<Vec<(Entity, f32)>>) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let _ = &mut base;
    for (mut tf, s) in &mut q {
        let local = tf.rotation * s.axis;
        tf.rotate(Quat::from_axis_angle(local, s.speed * dt));
        if s.bob > 0.0 {
            tf.translation.y += (t * 1.5).cos() * s.bob * dt * 1.5;
        }
    }
}

fn sway(time: Res<Time>, mut q: Query<(&mut Transform, &Sway)>) {
    let t = time.elapsed_secs();
    for (mut tf, s) in &mut q {
        let a = (t * 1.1 + s.phase).sin() * s.amount;
        let b = (t * 0.7 + s.phase * 1.3).sin() * s.amount * 0.6;
        tf.rotation = Quat::from_euler(EulerRot::XYZ, a, 0.0, b);
    }
}
