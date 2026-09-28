//! 共有メッシュ・マテリアル・フォント。

use bevy::prelude::*;

use crate::util::{flat, jittered_ico};

pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, setup_assets);
    }
}

#[derive(Resource)]
pub struct GameAssets {
    pub font: Handle<Font>,
    /// 頂点カラーをそのまま使うマテリアル
    pub vcol: Handle<StandardMaterial>,
    pub vcol_double: Handle<StandardMaterial>,

    pub sphere: Handle<Mesh>,
    pub sphere_lp: Handle<Mesh>,
    pub cube: Handle<Mesh>,
    pub capsule: Handle<Mesh>,
    pub cylinder: Handle<Mesh>,
    pub cone: Handle<Mesh>,
    pub torus: Handle<Mesh>,
    pub disc: Handle<Mesh>,
    pub ring: Handle<Mesh>,
    pub rocks: Vec<Handle<Mesh>>,
    pub blobs: Vec<Handle<Mesh>>,

    pub skin: Handle<StandardMaterial>,
    pub hair: Handle<StandardMaterial>,
    pub tunic: Handle<StandardMaterial>,
    pub leather: Handle<StandardMaterial>,
    pub cape: Handle<StandardMaterial>,
    pub steel: Handle<StandardMaterial>,
    pub gold: Handle<StandardMaterial>,
    pub blade_glow: Handle<StandardMaterial>,
    pub dark: Handle<StandardMaterial>,
    pub white_eye: Handle<StandardMaterial>,

    pub slime: Handle<StandardMaterial>,
    pub slime_core: Handle<StandardMaterial>,
    pub wolf_fur: Handle<StandardMaterial>,
    pub wolf_dark: Handle<StandardMaterial>,
    pub bone: Handle<StandardMaterial>,
    pub rust: Handle<StandardMaterial>,
    pub red_glow: Handle<StandardMaterial>,
    pub golem_stone: Handle<StandardMaterial>,
    pub golem_moss: Handle<StandardMaterial>,
    pub cyan_glow: Handle<StandardMaterial>,
    pub knight_armor: Handle<StandardMaterial>,
    pub knight_trim: Handle<StandardMaterial>,
    pub purple_glow: Handle<StandardMaterial>,
    pub knight_cape: Handle<StandardMaterial>,

    pub fx_spark: Handle<StandardMaterial>,
    pub fx_fire: Handle<StandardMaterial>,
    pub fx_heal: Handle<StandardMaterial>,
    pub fx_purple: Handle<StandardMaterial>,
    pub fx_dust: Handle<StandardMaterial>,
    pub fx_gold: Handle<StandardMaterial>,
    pub fx_blue: Handle<StandardMaterial>,
    pub fx_trail: Handle<StandardMaterial>,
    pub fx_warn: Handle<StandardMaterial>,
    pub fx_shock: Handle<StandardMaterial>,
    pub fx_blood: Handle<StandardMaterial>,
}

fn pbr(color: Color, rough: f32, metal: f32) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        perceptual_roughness: rough,
        metallic: metal,
        ..default()
    }
}

pub fn glow(color: Color, strength: f32) -> StandardMaterial {
    let lin = color.to_linear();
    StandardMaterial {
        base_color: color,
        emissive: LinearRgba::rgb(lin.red * strength, lin.green * strength, lin.blue * strength),
        unlit: false,
        fog_enabled: false,
        ..default()
    }
}

fn setup_assets(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let mut m = |mat: StandardMaterial| mats.add(mat);
    let srgb = Color::srgb;

    let ga = GameAssets {
        font: server.load("fonts/game.otf"),
        vcol: m(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.92,
            ..default()
        }),
        vcol_double: m(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.85,
            cull_mode: None,
            double_sided: true,
            ..default()
        }),

        sphere: meshes.add(Sphere::new(1.0).mesh().ico(3).unwrap()),
        sphere_lp: meshes.add(flat(Sphere::new(1.0).mesh().ico(1).unwrap())),
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        capsule: meshes.add(Capsule3d::new(0.5, 1.0)),
        cylinder: meshes.add(Cylinder::new(0.5, 1.0)),
        cone: meshes.add(flat(Cone::new(0.5, 1.0).mesh().resolution(7))),
        torus: meshes.add(Torus::new(0.85, 1.0)),
        disc: meshes.add(Circle::new(1.0)),
        ring: meshes.add(Annulus::new(0.85, 1.0)),
        rocks: (0..5)
            .map(|i| meshes.add(jittered_ico(1, 0.28, Vec3::new(1.0, 0.7, 1.0), 11 + i)))
            .collect(),
        blobs: (0..4)
            .map(|i| meshes.add(jittered_ico(1, 0.18, Vec3::ONE, 101 + i)))
            .collect(),

        skin: m(pbr(srgb(0.96, 0.76, 0.62), 0.7, 0.0)),
        hair: m(pbr(srgb(0.32, 0.2, 0.12), 0.8, 0.0)),
        tunic: m(pbr(srgb(0.16, 0.33, 0.66), 0.8, 0.0)),
        leather: m(pbr(srgb(0.36, 0.22, 0.12), 0.75, 0.0)),
        cape: m(StandardMaterial {
            cull_mode: None,
            double_sided: true,
            ..pbr(srgb(0.72, 0.12, 0.12), 0.85, 0.0)
        }),
        steel: m(pbr(srgb(0.78, 0.8, 0.86), 0.28, 0.9)),
        gold: m(pbr(srgb(0.95, 0.72, 0.25), 0.3, 1.0)),
        blade_glow: m(glow(srgb(0.55, 0.85, 1.0), 2.5)),
        dark: m(pbr(srgb(0.05, 0.05, 0.06), 0.5, 0.0)),
        white_eye: m(glow(Color::WHITE, 1.0)),

        slime: m(StandardMaterial {
            base_color: Color::srgba(0.35, 0.85, 0.35, 0.78),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.12,
            reflectance: 0.7,
            ..default()
        }),
        slime_core: m(glow(srgb(0.8, 1.0, 0.4), 1.5)),
        wolf_fur: m(pbr(srgb(0.45, 0.45, 0.5), 0.95, 0.0)),
        wolf_dark: m(pbr(srgb(0.2, 0.2, 0.24), 0.95, 0.0)),
        bone: m(pbr(srgb(0.88, 0.85, 0.76), 0.7, 0.0)),
        rust: m(pbr(srgb(0.45, 0.28, 0.18), 0.6, 0.6)),
        red_glow: m(glow(srgb(1.0, 0.15, 0.05), 6.0)),
        golem_stone: m(pbr(srgb(0.46, 0.44, 0.42), 0.95, 0.0)),
        golem_moss: m(pbr(srgb(0.3, 0.45, 0.2), 0.95, 0.0)),
        cyan_glow: m(glow(srgb(0.2, 0.9, 1.0), 5.0)),
        knight_armor: m(pbr(srgb(0.12, 0.11, 0.15), 0.35, 0.95)),
        knight_trim: m(pbr(srgb(0.55, 0.42, 0.2), 0.3, 1.0)),
        purple_glow: m(glow(srgb(0.7, 0.2, 1.0), 6.0)),
        knight_cape: m(StandardMaterial {
            cull_mode: None,
            double_sided: true,
            ..pbr(srgb(0.25, 0.05, 0.3), 0.85, 0.0)
        }),

        fx_spark: m(glow(srgb(1.0, 0.85, 0.5), 8.0)),
        fx_fire: m(glow(srgb(1.0, 0.45, 0.1), 7.0)),
        fx_heal: m(glow(srgb(0.4, 1.0, 0.55), 5.0)),
        fx_purple: m(glow(srgb(0.75, 0.3, 1.0), 6.0)),
        fx_dust: m(StandardMaterial {
            base_color: Color::srgba(0.75, 0.68, 0.55, 0.5),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 1.0,
            ..default()
        }),
        fx_gold: m(glow(srgb(1.0, 0.8, 0.2), 5.0)),
        fx_blue: m(glow(srgb(0.3, 0.7, 1.0), 6.0)),
        fx_trail: m(StandardMaterial {
            base_color: Color::srgba(0.6, 0.9, 1.0, 0.55),
            emissive: LinearRgba::rgb(1.5, 3.5, 6.0),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            cull_mode: None,
            fog_enabled: false,
            ..default()
        }),
        fx_warn: m(StandardMaterial {
            base_color: Color::srgba(1.0, 0.1, 0.05, 0.35),
            emissive: LinearRgba::rgb(3.0, 0.2, 0.1),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            cull_mode: None,
            fog_enabled: false,
            ..default()
        }),
        fx_shock: m(StandardMaterial {
            base_color: Color::srgba(0.8, 0.5, 1.0, 0.7),
            emissive: LinearRgba::rgb(3.0, 1.0, 5.0),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            cull_mode: None,
            fog_enabled: false,
            ..default()
        }),
        fx_blood: m(glow(srgb(1.0, 0.25, 0.2), 3.0)),
    };
    commands.insert_resource(ga);
}
