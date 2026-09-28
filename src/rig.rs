//! プリミティブで組んだキャラクターモデルと、手続き的アニメーション。

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;

use crate::{assets::GameAssets, util::damp};

type Mat = Handle<StandardMaterial>;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PoseAction {
    #[default]
    None,
    Slash1,
    Slash2,
    Slam,
    Thrust,
    Roll,
    Drink,
    Cast,
    Leap,
    Bite,
    Windup,
    Talk,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RigStyle {
    #[default]
    Humanoid,
    Quadruped,
    Slime,
}

/// ルートに付けるポーズ状態。各パーツ (Limb) がこれを読んで姿勢を決める。
#[derive(Component, Default, Clone)]
pub struct Pose {
    pub style: RigStyle,
    pub phase: f32,
    pub moving: f32,
    pub action: PoseAction,
    /// 行動の進行度 0..1
    pub t: f32,
    pub air: bool,
    pub hurt: f32,
    pub dead: f32,
    pub clock: f32,
    pub two_hand: bool,
    /// 攻撃予兆の光 (0..1)
    pub charge: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LimbKind {
    Body,
    Head,
    ArmR,
    ArmL,
    LegR,
    LegL,
    Cape,
    QuadLeg(u8),
    Tail,
    SlimeBody,
}

#[derive(Component)]
pub struct Limb {
    pub kind: LimbKind,
    pub owner: Entity,
    pub rest: Vec3,
}

/// 剣の先端/根元 (斬撃トレイル用)。
#[derive(Component)]
pub struct SwordTip(pub Entity);
#[derive(Component)]
pub struct SwordBase(pub Entity);
/// 装備変更で色が変わる刃の発光部分。
#[derive(Component)]
pub struct BladeEdge(pub Entity);
#[derive(Component)]
pub struct BladeMesh(pub Entity);

pub struct WeaponLook {
    pub blade: Mat,
    pub edge: Mat,
    pub hilt: Mat,
    pub length: f32,
    pub width: f32,
}

pub struct Look {
    pub scale: f32,
    pub bulk: f32,
    pub skin: Mat,
    pub torso: Mat,
    pub legs: Mat,
    pub arms: Mat,
    pub boots: Mat,
    pub hair: Option<Mat>,
    pub helmet: Option<Mat>,
    pub eyes: Mat,
    pub cape: Option<Mat>,
    pub weapon: Option<WeaponLook>,
    pub shield: Option<(Mat, Mat)>,
    pub beard: Option<Mat>,
    pub hat: Option<Mat>,
    pub pauldrons: Option<Mat>,
    pub horns: Option<Mat>,
    pub staff: Option<Mat>,
    pub apron: Option<Mat>,
}

impl Look {
    pub fn basic(ga: &GameAssets) -> Self {
        Look {
            scale: 1.0,
            bulk: 1.0,
            skin: ga.skin.clone(),
            torso: ga.tunic.clone(),
            legs: ga.leather.clone(),
            arms: ga.tunic.clone(),
            boots: ga.leather.clone(),
            hair: Some(ga.hair.clone()),
            helmet: None,
            eyes: ga.dark.clone(),
            cape: None,
            weapon: None,
            shield: None,
            beard: None,
            hat: None,
            pauldrons: None,
            horns: None,
            staff: None,
            apron: None,
        }
    }
}

fn limb(kind: LimbKind, owner: Entity, t: Transform) -> (Limb, Transform, Visibility) {
    (Limb { kind, owner, rest: t.translation }, t, Visibility::default())
}

fn part(mesh: &Handle<Mesh>, mat: &Mat, t: Transform) -> (Mesh3d, MeshMaterial3d<StandardMaterial>, Transform) {
    (Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), t)
}

/// 人型モデルを owner の子として生成する。
pub fn build_humanoid(commands: &mut Commands, ga: &GameAssets, owner: Entity, look: &Look) {
    let b = look.bulk;
    let thin = b.min(1.0);
    let hip_y = 0.9;
    let visual = commands
        .spawn((Transform::from_scale(Vec3::splat(look.scale)), Visibility::default()))
        .id();
    commands.entity(owner).add_child(visual);

    let body = commands.spawn(limb(LimbKind::Body, owner, Transform::from_xyz(0.0, hip_y, 0.0))).id();
    commands.entity(visual).add_child(body);

    let mut kids = Vec::new();
    // 胴体
    kids.push(
        commands
            .spawn(part(
                &ga.capsule,
                &look.torso,
                Transform::from_xyz(0.0, 0.34, 0.0).with_scale(Vec3::new(0.5 * b, 0.36, 0.36 * b)),
            ))
            .id(),
    );
    kids.push(
        commands
            .spawn(part(
                &ga.cylinder,
                &ga.leather,
                Transform::from_xyz(0.0, 0.1, 0.0).with_scale(Vec3::new(0.52 * b, 0.09, 0.4 * b)),
            ))
            .id(),
    );
    kids.push(
        commands
            .spawn(part(
                &ga.cone,
                look.apron.as_ref().unwrap_or(&look.torso),
                Transform::from_xyz(0.0, 0.02, 0.0)
                    .with_rotation(Quat::from_rotation_x(PI))
                    .with_scale(Vec3::new(0.62 * b, 0.28, 0.5 * b)),
            ))
            .id(),
    );
    if let Some(m) = &look.pauldrons {
        for sx in [-1.0, 1.0] {
            kids.push(
                commands
                    .spawn(part(&ga.sphere_lp, m, Transform::from_xyz(sx * 0.3 * b, 0.56, 0.0).with_scale(Vec3::splat(0.16 * b))))
                    .id(),
            );
        }
    }

    // 頭
    let head = commands.spawn(limb(LimbKind::Head, owner, Transform::from_xyz(0.0, 0.68, 0.0))).id();
    let mut hk = vec![
        commands
            .spawn(part(&ga.sphere, &look.skin, Transform::from_xyz(0.0, 0.14, 0.0).with_scale(Vec3::splat(0.2))))
            .id(),
    ];
    for sx in [-1.0, 1.0] {
        hk.push(
            commands
                .spawn(part(&ga.sphere, &look.eyes, Transform::from_xyz(sx * 0.07, 0.15, -0.175).with_scale(Vec3::splat(0.032))))
                .id(),
        );
    }
    if let Some(h) = &look.hair {
        hk.push(
            commands
                .spawn(part(&ga.sphere, h, Transform::from_xyz(0.0, 0.2, 0.035).with_scale(Vec3::new(0.215, 0.17, 0.2))))
                .id(),
        );
        hk.push(
            commands
                .spawn(part(&ga.sphere_lp, h, Transform::from_xyz(0.05, 0.28, -0.1).with_scale(Vec3::new(0.14, 0.07, 0.1))))
                .id(),
        );
    }
    if let Some(h) = &look.helmet {
        hk.push(
            commands
                .spawn(part(&ga.sphere, h, Transform::from_xyz(0.0, 0.19, 0.01).with_scale(Vec3::new(0.235, 0.21, 0.235))))
                .id(),
        );
        hk.push(
            commands
                .spawn(part(&ga.cube, h, Transform::from_xyz(0.0, 0.1, -0.2).with_scale(Vec3::new(0.3, 0.05, 0.06))))
                .id(),
        );
    }
    if let Some(m) = &look.horns {
        for sx in [-1.0, 1.0] {
            hk.push(
                commands
                    .spawn(part(
                        &ga.cone,
                        m,
                        Transform::from_xyz(sx * 0.2, 0.32, 0.0)
                            .with_rotation(Quat::from_rotation_z(-sx * 0.7))
                            .with_scale(Vec3::new(0.08, 0.3, 0.08)),
                    ))
                    .id(),
            );
        }
    }
    if let Some(m) = &look.beard {
        hk.push(
            commands
                .spawn(part(
                    &ga.cone,
                    m,
                    Transform::from_xyz(0.0, 0.0, -0.12)
                        .with_rotation(Quat::from_rotation_x(PI + 0.25))
                        .with_scale(Vec3::new(0.2, 0.32, 0.12)),
                ))
                .id(),
        );
    }
    if let Some(m) = &look.hat {
        hk.push(
            commands
                .spawn(part(&ga.cylinder, m, Transform::from_xyz(0.0, 0.26, 0.0).with_scale(Vec3::new(0.52, 0.03, 0.52))))
                .id(),
        );
        hk.push(
            commands
                .spawn(part(&ga.cone, m, Transform::from_xyz(0.0, 0.42, 0.02).with_scale(Vec3::new(0.3, 0.36, 0.3))))
                .id(),
        );
    }
    commands.entity(head).add_children(&hk);
    kids.push(head);

    // 腕
    for (kind, sx) in [(LimbKind::ArmR, 1.0), (LimbKind::ArmL, -1.0)] {
        let arm = commands
            .spawn(limb(kind, owner, Transform::from_xyz(sx * 0.3 * b, 0.55, 0.0)))
            .id();
        let mut ak = vec![
            commands
                .spawn(part(&ga.capsule, &look.arms, Transform::from_xyz(0.0, -0.27, 0.0).with_scale(Vec3::new(0.15 * thin.max(0.6) * b.max(1.0), 0.3, 0.15 * thin.max(0.6) * b.max(1.0)))))
                .id(),
            commands
                .spawn(part(&ga.sphere, &look.skin, Transform::from_xyz(0.0, -0.56, 0.0).with_scale(Vec3::splat(0.075 * b.max(1.0)))))
                .id(),
        ];
        if kind == LimbKind::ArmR
            && let Some(w) = &look.weapon
        {
            let sword = commands
                .spawn((
                    Transform::from_xyz(0.0, -0.58, 0.0).with_rotation(Quat::from_rotation_x(0.35)),
                    Visibility::default(),
                ))
                .id();
            let l = w.length;
            let sk = vec![
                commands.spawn(part(&ga.cylinder, &w.hilt, Transform::from_xyz(0.0, 0.05, 0.0).with_scale(Vec3::new(0.06, 0.24, 0.06)))).id(),
                commands.spawn(part(&ga.sphere, &ga.gold, Transform::from_xyz(0.0, 0.19, 0.0).with_scale(Vec3::splat(0.05)))).id(),
                commands.spawn(part(&ga.cube, &ga.gold, Transform::from_xyz(0.0, -0.08, 0.0).with_scale(Vec3::new(0.3 * w.width.max(1.0), 0.05, 0.08)))).id(),
                commands
                    .spawn((
                        part(&ga.cube, &w.blade, Transform::from_xyz(0.0, -0.1 - l / 2.0, 0.0).with_scale(Vec3::new(0.075 * w.width, l, 0.022))),
                        BladeMesh(owner),
                    ))
                    .id(),
                commands
                    .spawn((
                        part(&ga.cube, &w.edge, Transform::from_xyz(0.0, -0.1 - l / 2.0, 0.0).with_scale(Vec3::new(0.025 * w.width, l * 0.94, 0.03))),
                        BladeEdge(owner),
                    ))
                    .id(),
                commands.spawn((Transform::from_xyz(0.0, -0.1 - l, 0.0), SwordTip(owner))).id(),
                commands.spawn((Transform::from_xyz(0.0, -0.25, 0.0), SwordBase(owner))).id(),
            ];
            commands.entity(sword).add_children(&sk);
            ak.push(sword);
        }
        if kind == LimbKind::ArmR
            && let Some(m) = &look.staff
        {
            ak.push(
                commands
                    .spawn(part(&ga.cylinder, m, Transform::from_xyz(0.0, -0.55, -0.05).with_scale(Vec3::new(0.06, 1.7, 0.06))))
                    .id(),
            );
            ak.push(
                commands
                    .spawn(part(&ga.sphere_lp, &ga.fx_blue, Transform::from_xyz(0.0, 0.33, -0.05).with_scale(Vec3::splat(0.09))))
                    .id(),
            );
        }
        if kind == LimbKind::ArmL
            && let Some((face, rim)) = &look.shield
        {
            ak.push(
                commands
                    .spawn(part(
                        &ga.cylinder,
                        rim,
                        Transform::from_xyz(-0.1, -0.36, 0.0)
                            .with_rotation(Quat::from_rotation_z(PI / 2.0))
                            .with_scale(Vec3::new(0.62, 0.05, 0.62)),
                    ))
                    .id(),
            );
            ak.push(
                commands
                    .spawn(part(
                        &ga.cylinder,
                        face,
                        Transform::from_xyz(-0.13, -0.36, 0.0)
                            .with_rotation(Quat::from_rotation_z(PI / 2.0))
                            .with_scale(Vec3::new(0.52, 0.03, 0.52)),
                    ))
                    .id(),
            );
        }
        commands.entity(arm).add_children(&ak);
        kids.push(arm);
    }

    // 脚
    for (kind, sx) in [(LimbKind::LegR, 1.0), (LimbKind::LegL, -1.0)] {
        let leg = commands
            .spawn(limb(kind, owner, Transform::from_xyz(sx * 0.14 * b, 0.02, 0.0)))
            .id();
        let lk = vec![
            commands
                .spawn(part(&ga.capsule, &look.legs, Transform::from_xyz(0.0, -0.4, 0.0).with_scale(Vec3::new(0.19 * thin.max(0.6) * b.max(1.0), 0.52, 0.19 * thin.max(0.6) * b.max(1.0)))))
                .id(),
            commands
                .spawn(part(&ga.cube, &look.boots, Transform::from_xyz(0.0, -0.8, -0.05).with_scale(Vec3::new(0.18 * b.max(1.0), 0.14, 0.3 * b.max(1.0)))))
                .id(),
        ];
        commands.entity(leg).add_children(&lk);
        kids.push(leg);
    }

    if let Some(m) = &look.cape {
        let cape = commands
            .spawn(limb(LimbKind::Cape, owner, Transform::from_xyz(0.0, 0.6, 0.19 * b)))
            .id();
        let c = commands
            .spawn(part(&ga.cube, m, Transform::from_xyz(0.0, -0.45, 0.0).with_scale(Vec3::new(0.52 * b, 0.9, 0.03))))
            .id();
        commands.entity(cape).add_child(c);
        kids.push(cape);
    }
    commands.entity(body).add_children(&kids);
}

/// 四足の獣 (狼)。
pub fn build_quadruped(commands: &mut Commands, ga: &GameAssets, owner: Entity, fur: &Mat, dark: &Mat, eye: &Mat, scale: f32) {
    let visual = commands
        .spawn((Transform::from_scale(Vec3::splat(scale)), Visibility::default()))
        .id();
    commands.entity(owner).add_child(visual);
    let body = commands.spawn(limb(LimbKind::Body, owner, Transform::from_xyz(0.0, 0.75, 0.0))).id();
    commands.entity(visual).add_child(body);
    let mut kids = vec![
        commands
            .spawn(part(
                &ga.capsule,
                fur,
                Transform::from_rotation(Quat::from_rotation_x(PI / 2.0)).with_scale(Vec3::new(0.5, 0.75, 0.48)),
            ))
            .id(),
        commands
            .spawn(part(&ga.sphere_lp, dark, Transform::from_xyz(0.0, 0.12, -0.3).with_scale(Vec3::new(0.3, 0.28, 0.35))))
            .id(),
    ];
    let head = commands.spawn(limb(LimbKind::Head, owner, Transform::from_xyz(0.0, 0.22, -0.62))).id();
    let hk = vec![
        commands.spawn(part(&ga.sphere_lp, fur, Transform::from_xyz(0.0, 0.05, -0.05).with_scale(Vec3::new(0.24, 0.22, 0.26)))).id(),
        commands.spawn(part(&ga.cube, fur, Transform::from_xyz(0.0, -0.02, -0.3).with_scale(Vec3::new(0.16, 0.14, 0.3)))).id(),
        commands.spawn(part(&ga.sphere, &ga.dark, Transform::from_xyz(0.0, 0.02, -0.46).with_scale(Vec3::splat(0.045)))).id(),
        commands.spawn(part(&ga.cone, dark, Transform::from_xyz(0.1, 0.25, 0.0).with_scale(Vec3::new(0.1, 0.18, 0.08)))).id(),
        commands.spawn(part(&ga.cone, dark, Transform::from_xyz(-0.1, 0.25, 0.0).with_scale(Vec3::new(0.1, 0.18, 0.08)))).id(),
        commands.spawn(part(&ga.sphere, eye, Transform::from_xyz(0.09, 0.08, -0.2).with_scale(Vec3::splat(0.035)))).id(),
        commands.spawn(part(&ga.sphere, eye, Transform::from_xyz(-0.09, 0.08, -0.2).with_scale(Vec3::splat(0.035)))).id(),
    ];
    commands.entity(head).add_children(&hk);
    kids.push(head);
    for i in 0..4u8 {
        let sx = if i % 2 == 0 { 1.0 } else { -1.0 };
        let sz = if i < 2 { -0.38 } else { 0.38 };
        let leg = commands.spawn(limb(LimbKind::QuadLeg(i), owner, Transform::from_xyz(sx * 0.16, -0.1, sz))).id();
        let l = commands
            .spawn(part(&ga.capsule, fur, Transform::from_xyz(0.0, -0.3, 0.0).with_scale(Vec3::new(0.12, 0.45, 0.12))))
            .id();
        let paw = commands
            .spawn(part(&ga.sphere_lp, dark, Transform::from_xyz(0.0, -0.6, -0.03).with_scale(Vec3::new(0.08, 0.06, 0.1))))
            .id();
        commands.entity(leg).add_children(&[l, paw]);
        kids.push(leg);
    }
    let tail = commands.spawn(limb(LimbKind::Tail, owner, Transform::from_xyz(0.0, 0.1, 0.45))).id();
    let tm = commands
        .spawn(part(
            &ga.capsule,
            dark,
            Transform::from_xyz(0.0, 0.1, 0.22).with_rotation(Quat::from_rotation_x(1.1)).with_scale(Vec3::new(0.09, 0.4, 0.09)),
        ))
        .id();
    commands.entity(tail).add_child(tm);
    kids.push(tail);
    commands.entity(body).add_children(&kids);
}

/// スライム。
pub fn build_slime(commands: &mut Commands, ga: &GameAssets, owner: Entity, scale: f32) {
    let visual = commands
        .spawn((Transform::from_scale(Vec3::splat(scale)), Visibility::default()))
        .id();
    commands.entity(owner).add_child(visual);
    let body = commands.spawn(limb(LimbKind::SlimeBody, owner, Transform::from_xyz(0.0, 0.0, 0.0))).id();
    let k = vec![
        commands.spawn(part(&ga.sphere, &ga.slime, Transform::from_xyz(0.0, 0.5, 0.0).with_scale(Vec3::new(0.62, 0.5, 0.62)))).id(),
        commands.spawn(part(&ga.sphere_lp, &ga.slime_core, Transform::from_xyz(0.0, 0.45, 0.0).with_scale(Vec3::splat(0.18)))).id(),
        commands.spawn(part(&ga.sphere, &ga.dark, Transform::from_xyz(0.17, 0.62, -0.5).with_scale(Vec3::new(0.07, 0.1, 0.05)))).id(),
        commands.spawn(part(&ga.sphere, &ga.dark, Transform::from_xyz(-0.17, 0.62, -0.5).with_scale(Vec3::new(0.07, 0.1, 0.05)))).id(),
        commands.spawn(part(&ga.sphere, &ga.white_eye, Transform::from_xyz(0.19, 0.65, -0.53).with_scale(Vec3::splat(0.022)))).id(),
        commands.spawn(part(&ga.sphere, &ga.white_eye, Transform::from_xyz(-0.15, 0.65, -0.53).with_scale(Vec3::splat(0.022)))).id(),
    ];
    commands.entity(body).add_children(&k);
    commands.entity(visual).add_child(body);
}

/// キーフレーム補間 (区間ごとに smoothstep)。
pub fn keys(t: f32, k: &[(f32, f32)]) -> f32 {
    if t <= k[0].0 {
        return k[0].1;
    }
    for w in k.windows(2) {
        let (t0, v0) = w[0];
        let (t1, v1) = w[1];
        if t <= t1 {
            let u = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
            let u = u * u * (3.0 - 2.0 * u);
            return v0 + (v1 - v0) * u;
        }
    }
    k[k.len() - 1].1
}

struct ArmPose {
    yaw: f32,
    pitch: f32,
    roll: f32,
}

fn arm_pose(p: &Pose, right: bool) -> ArmPose {
    let side = if right { 1.0 } else { -1.0 };
    let walk = (p.phase + if right { 0.0 } else { PI }).sin() * 0.55 * p.moving;
    let breathe = (p.clock * 2.0).sin() * 0.03;
    let mut a = ArmPose {
        yaw: -0.15 * side,
        pitch: if right { 0.45 } else { 0.15 } - walk + breathe,
        roll: 0.14 * side,
    };
    let t = p.t;
    let affects = right || p.two_hand;
    match p.action {
        PoseAction::Slash1 if affects => {
            a.pitch = keys(t, &[(0.0, 0.5), (0.28, 1.35), (0.55, 1.45), (1.0, 0.6)]);
            a.yaw = keys(t, &[(0.0, -0.3), (0.28, -1.7), (0.55, 1.2), (1.0, -0.2)]) * if right { 1.0 } else { -1.0 };
            a.roll = 0.1 * side;
        }
        PoseAction::Slash2 if affects => {
            a.pitch = keys(t, &[(0.0, 1.0), (0.28, 1.3), (0.55, 1.4), (1.0, 0.6)]);
            a.yaw = keys(t, &[(0.0, 0.8), (0.28, 1.3), (0.55, -1.5), (1.0, -0.2)]) * if right { 1.0 } else { -1.0 };
        }
        PoseAction::Slam | PoseAction::Leap if affects => {
            a.pitch = keys(t, &[(0.0, 0.5), (0.4, 3.1), (0.58, 0.55), (1.0, 0.5)]);
            a.yaw = -0.05 * side;
            a.roll = if p.two_hand { 0.3 * side } else { 0.05 };
        }
        PoseAction::Thrust if right => {
            a.pitch = keys(t, &[(0.0, 0.8), (0.35, 1.2), (0.5, 1.6), (1.0, 0.7)]);
            a.yaw = keys(t, &[(0.0, -0.2), (0.35, -0.6), (0.5, 0.0), (1.0, -0.1)]);
        }
        PoseAction::Windup if affects => {
            a.pitch = 1.2 + p.charge * 1.4;
            a.yaw = -0.6 * side;
        }
        PoseAction::Roll => {
            a.pitch = 1.2;
        }
        PoseAction::Drink if !right => {
            a.pitch = keys(t, &[(0.0, 0.2), (0.3, 2.5), (0.8, 2.5), (1.0, 0.2)]);
            a.yaw = 0.5;
            a.roll = -0.5;
        }
        PoseAction::Cast if !right => {
            a.pitch = keys(t, &[(0.0, 0.3), (0.4, 1.9), (0.8, 1.9), (1.0, 0.3)]);
            a.roll = -0.4;
        }
        PoseAction::Talk => {
            a.pitch = 0.3 + (p.clock * 3.0 + side).sin().abs() * 0.5;
            a.roll = 0.3 * side;
        }
        _ => {}
    }
    if p.air {
        a.pitch += 0.3;
        a.roll += 0.3 * side;
    }
    a
}

fn body_rot(p: &Pose) -> (Quat, Vec3) {
    let t = p.t;
    let bob = (p.phase * 2.0).sin().abs() * 0.06 * p.moving;
    let mut off = Vec3::new(0.0, bob, 0.0);
    let mut pitch = -0.12 * p.moving;
    let mut yaw = 0.0;
    match p.action {
        PoseAction::Slash1 => yaw = keys(t, &[(0.0, 0.0), (0.28, -0.45), (0.55, 0.5), (1.0, 0.0)]),
        PoseAction::Slash2 => yaw = keys(t, &[(0.0, 0.2), (0.28, 0.45), (0.55, -0.5), (1.0, 0.0)]),
        PoseAction::Slam | PoseAction::Leap => {
            pitch = keys(t, &[(0.0, 0.0), (0.4, 0.25), (0.58, -0.45), (1.0, 0.0)]);
            off.y -= keys(t, &[(0.0, 0.0), (0.4, 0.0), (0.58, 0.15), (1.0, 0.0)]);
        }
        PoseAction::Thrust => pitch = keys(t, &[(0.0, 0.0), (0.35, 0.15), (0.5, -0.3), (1.0, 0.0)]),
        PoseAction::Windup => pitch = 0.15 + 0.1 * p.charge,
        PoseAction::Roll => {
            pitch = -TAU * t;
            off.y -= (PI * t).sin() * 0.45;
        }
        PoseAction::Bite => pitch = keys(t, &[(0.0, 0.0), (0.3, 0.2), (0.5, -0.25), (1.0, 0.0)]),
        _ => {}
    }
    pitch += p.hurt * 0.35;
    if p.dead > 0.0 {
        pitch = p.dead * 1.45;
        off.y -= p.dead * 0.65;
    }
    (Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0), off)
}

pub fn animate_limbs(
    time: Res<Time>,
    poses: Query<&Pose>,
    mut limbs: Query<(&Limb, &mut Transform)>,
) {
    let dt = time.delta_secs();
    let k = damp(22.0, dt);
    for (l, mut tf) in &mut limbs {
        let Ok(p) = poses.get(l.owner) else { continue };
        let (rot, off) = match (p.style, l.kind) {
            (RigStyle::Humanoid, LimbKind::Body) => body_rot(p),
            (RigStyle::Humanoid, LimbKind::Head) => {
                let nod = (p.clock * 1.7).sin() * 0.03;
                let tilt = if p.action == PoseAction::Drink { -0.5 } else { 0.0 };
                (Quat::from_euler(EulerRot::YXZ, 0.0, nod + tilt + p.hurt * -0.3, 0.0), Vec3::ZERO)
            }
            (RigStyle::Humanoid, LimbKind::ArmR | LimbKind::ArmL) => {
                let a = arm_pose(p, l.kind == LimbKind::ArmR);
                (Quat::from_euler(EulerRot::YXZ, a.yaw, a.pitch, a.roll), Vec3::ZERO)
            }
            (RigStyle::Humanoid, LimbKind::LegR | LimbKind::LegL) => {
                let s = if l.kind == LimbKind::LegR { 0.0 } else { PI };
                let mut pitch = (p.phase + s).sin() * 0.75 * p.moving;
                if p.air {
                    pitch = if l.kind == LimbKind::LegR { 0.6 } else { -0.25 };
                }
                if p.action == PoseAction::Roll {
                    pitch = 1.4;
                }
                if matches!(p.action, PoseAction::Slam | PoseAction::Leap | PoseAction::Thrust) {
                    let lunge = keys(p.t, &[(0.0, 0.0), (0.45, 0.0), (0.58, 0.55), (1.0, 0.0)]);
                    pitch = if l.kind == LimbKind::LegR { lunge } else { -lunge * 0.6 };
                }
                (Quat::from_rotation_x(pitch), Vec3::ZERO)
            }
            (_, LimbKind::Cape) => {
                let flap = (p.clock * 7.0).sin() * 0.06 * (0.3 + p.moving);
                (Quat::from_rotation_x(-0.12 - p.moving * 0.55 + flap), Vec3::ZERO)
            }
            (RigStyle::Quadruped, LimbKind::Body) => {
                let bob = (p.phase * 2.0).sin() * 0.05 * p.moving;
                let mut pitch = (p.phase * 2.0).cos() * 0.05 * p.moving;
                let mut off = Vec3::Y * bob;
                if p.action == PoseAction::Bite {
                    pitch += keys(p.t, &[(0.0, 0.0), (0.3, -0.25), (0.5, 0.2), (1.0, 0.0)]);
                }
                if p.action == PoseAction::Windup {
                    off.y -= 0.12 * p.charge;
                    pitch -= 0.15 * p.charge;
                }
                pitch += p.hurt * 0.3;
                if p.dead > 0.0 {
                    off.y -= p.dead * 0.45;
                    return_roll(&mut tf, l, p.dead, off);
                    continue;
                }
                (Quat::from_rotation_x(pitch), off)
            }
            (RigStyle::Quadruped, LimbKind::Head) => {
                let mut pitch = (p.clock * 2.0).sin() * 0.05;
                if p.action == PoseAction::Bite {
                    pitch += keys(p.t, &[(0.0, 0.0), (0.3, -0.4), (0.5, 0.35), (1.0, 0.0)]);
                }
                (Quat::from_rotation_x(pitch), Vec3::ZERO)
            }
            (RigStyle::Quadruped, LimbKind::QuadLeg(i)) => {
                let off = if i == 0 || i == 3 { 0.0 } else { PI };
                let pitch = (p.phase + off).sin() * 0.8 * p.moving;
                (Quat::from_rotation_x(pitch), Vec3::ZERO)
            }
            (RigStyle::Quadruped, LimbKind::Tail) => (
                Quat::from_rotation_y((p.clock * (4.0 + p.moving * 8.0)).sin() * 0.35),
                Vec3::ZERO,
            ),
            (RigStyle::Slime, LimbKind::SlimeBody) => {
                let hop = (p.phase).sin();
                let mut sq = 1.0 + hop * 0.12 * p.moving.max(0.3);
                let mut y = hop.max(0.0) * 0.35 * p.moving;
                if p.action == PoseAction::Windup {
                    sq = 1.0 - 0.3 * p.charge;
                }
                if p.action == PoseAction::Slam {
                    sq = keys(p.t, &[(0.0, 0.7), (0.3, 1.35), (0.6, 0.8), (1.0, 1.0)]);
                    y = keys(p.t, &[(0.0, 0.0), (0.3, 0.8), (0.6, 0.0), (1.0, 0.0)]);
                }
                if p.dead > 0.0 {
                    sq = 1.0 - p.dead * 0.8;
                }
                tf.translation = Vec3::Y * y;
                let w = 1.0 / sq.sqrt();
                tf.scale = Vec3::new(w, sq, w);
                continue;
            }
            _ => (Quat::IDENTITY, Vec3::ZERO),
        };
        tf.rotation = tf.rotation.slerp(rot, k);
        tf.translation = tf.translation.lerp(l.rest + off, k);
    }
}

fn return_roll(tf: &mut Transform, l: &Limb, dead: f32, off: Vec3) {
    tf.rotation = Quat::from_rotation_z(dead * 1.5);
    tf.translation = l.rest + off;
}
