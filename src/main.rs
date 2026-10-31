//! 蒼剣のエルディア — Bevy + WebAssembly 製の 3D アクション RPG。
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod assets;
mod audio;
mod camera;
mod combat;
mod dungeon;
mod enemy;
mod field;
mod input;
mod npc;
mod player;
mod render;
mod rig;
mod save;
mod stats;
mod terrain;
mod ui;
mod util;
mod vfx;
mod zone;

use bevy::{prelude::*, window::WindowResolution};

#[derive(States, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum GameState {
    /// アセット準備前の起動直後
    #[default]
    Boot,
    Title,
    Playing,
    Paused,
    GameOver,
    Victory,
}

/// 会話・インベントリなどでゲームプレイ入力を止めているか。
#[derive(Resource, Default)]
pub struct UiBlock {
    pub dialog: bool,
    pub inventory: bool,
    pub shop: bool,
}

impl UiBlock {
    pub fn any(&self) -> bool {
        self.dialog || self.inventory || self.shop
    }
}

/// プレイヤーが操作可能なフレームだけ動くシステム用の条件。
pub fn gameplay_active(state: Res<State<GameState>>, block: Res<UiBlock>) -> bool {
    *state.get() == GameState::Playing && !block.any()
}

/// 世界の時間が進むか (会話中などは敵も止まる)。
pub fn world_running(state: Res<State<GameState>>, block: Res<UiBlock>) -> bool {
    matches!(state.get(), GameState::Playing | GameState::Title | GameState::Boot) && !block.any()
}

#[cfg(target_arch = "wasm32")]
fn window_resolution() -> WindowResolution {
    let scale = web_sys::window().map_or(1.0, |w| w.device_pixel_ratio()) as f32;
    WindowResolution::new(1280, 720).with_scale_factor_override(scale)
}

#[cfg(not(target_arch = "wasm32"))]
fn window_resolution() -> WindowResolution {
    WindowResolution::new(1280, 720)
}

fn main() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "蒼剣のエルディア".into(),
                    canvas: Some("#game".into()),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: true,
                    // 描画の倍率は index.html が決める (高 DPI 画面でそのまま描くと重い)
                    resolution: window_resolution(),
                    ..default()
                }),
                ..default()
            })
            .set(AssetPlugin {
                meta_check: bevy::asset::AssetMetaCheck::Never,
                ..default()
            }),
    )
    .init_state::<GameState>()
    .init_resource::<UiBlock>()
    .init_resource::<util::GameRng>()
    .add_plugins((
        assets::AssetsPlugin,
        input::InputPlugin,
        render::RenderSetupPlugin,
        zone::ZonePlugin,
        player::PlayerPlugin,
        camera::CameraPlugin,
        stats::StatsPlugin,
        enemy::EnemyPlugin,
        combat::CombatPlugin,
        npc::NpcPlugin,
        vfx::VfxPlugin,
        audio::GameAudioPlugin,
        ui::UiPlugin,
        save::SavePlugin,
    ))
    .add_systems(Update, (hide_loading_screen, boot.run_if(in_state(GameState::Boot))));
    #[cfg(feature = "dev")]
    app.add_systems(Update, debug_keys);
    app.run();
}

fn boot(mut next: ResMut<NextState<GameState>>) {
    next.set(GameState::Title);
}

/// 地形生成などが終わって描画が始まったら HTML のローディング表示を消す。
fn hide_loading_screen(mut frames: Local<u32>) {
    *frames += 1;
    if *frames != 4 {
        return;
    }
    #[cfg(target_arch = "wasm32")]
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("loading"))
    {
        let _ = el.set_attribute("class", "done");
    }
}

/// 動作確認用のワープ (dev フィーチャー時のみ)。
#[cfg(feature = "dev")]
fn debug_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut req: MessageWriter<zone::ZoneRequest>,
    mut stats: ResMut<stats::PlayerStats>,
    mut progress: ResMut<stats::Progress>,
    mut commands: Commands,
    ga: Res<assets::GameAssets>,
    player: Query<&Transform, With<player::Player>>,
) {
    use bevy::math::Vec3;
    if keys.just_pressed(KeyCode::F7)
        && let Ok(t) = player.single()
    {
        let p = t.translation + t.forward().as_vec3() * 2.2;
        let kind = if keys.pressed(KeyCode::ShiftLeft) { enemy::EnemyKind::Golem } else { enemy::EnemyKind::Slime };
        enemy::spawn_enemy(&mut commands, &ga, kind, p, 1.0);
    }
    if keys.just_pressed(KeyCode::F8) {
        req.write(zone::ZoneRequest { zone: zone::ZoneId::Field, spawn: Vec3::new(30.0, 0.0, 16.0), facing: 0.0 });
    }
    if keys.just_pressed(KeyCode::F9) {
        req.write(zone::ZoneRequest { zone: zone::ZoneId::Dungeon, spawn: Vec3::new(82.0, 0.0, 38.0), facing: 0.0 });
    }
    if keys.just_pressed(KeyCode::F10) {
        stats.add_exp(3000);
        stats.weapons[2] = true;
        stats.weapon = 2;
        progress.main = stats::MainQuest::GoToRuins;
    }
}
