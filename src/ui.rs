//! HUD・メニュー・会話ウィンドウなどの UI。

use std::collections::{HashMap, VecDeque};

use bevy::{ecs::system::SystemParam, prelude::*, ui::widget::TextShadow};

use crate::{
    GameState, UiBlock,
    assets::GameAssets,
    audio::{Music, PlaySfx, SfxKind},
    camera::{CamRig, MainCamera},
    dungeon,
    enemy::{AiState, Boss, BossArena, Enemy, EnemyKind},
    field::VILLAGE_SPAWN,
    input::Controls,
    npc::{Dialog, Interactable, InteractKind, NearInteract, Npc, Transition, fade_alpha},
    player::{Player, PlayerCtl, revive},
    save,
    stats::{ARMORS, PlayerStats, Progress, WEAPONS},
    terrain,
    zone::{Colliders, CurrentZone, ZoneEntered, ZoneId, ZoneRequest},
};

// ---------------------------------------------------------------- 公開型

#[derive(Message, Clone)]
pub struct Banner {
    pub title: String,
    pub sub: String,
    pub big: bool,
}

impl Banner {
    pub fn big(title: &str, sub: &str) -> Self {
        Self { title: title.into(), sub: sub.into(), big: true }
    }
    pub fn small(title: &str) -> Self {
        Self { title: title.into(), sub: String::new(), big: false }
    }
}

/// 被ダメージ時の赤いフラッシュ (0..1)。
#[derive(Resource, Default)]
pub struct HurtFlash(pub f32);

/// ワールド座標に浮かぶ数字。
#[derive(Component)]
pub struct DamageNumber {
    pub world: Vec3,
    pub text: String,
    pub color: Color,
    pub size: f32,
    pub t: f32,
    pub drift: f32,
}

impl DamageNumber {
    pub fn new(world: Vec3, text: String, color: Color, size: f32) -> Self {
        let drift = ((world.x * 13.7 + world.z * 7.1).sin()) * 18.0;
        Self { world, text, color, size, t: 0.0, drift }
    }
}

// ---------------------------------------------------------------- 色とスタイル

const PANEL: Color = Color::srgba(0.04, 0.05, 0.09, 0.78);
const PANEL_BORDER: Color = Color::srgba(0.85, 0.7, 0.4, 0.55);
const GOLD: Color = Color::srgb(0.98, 0.84, 0.5);
const TEXT: Color = Color::srgb(0.95, 0.94, 0.9);
const DIM: Color = Color::srgb(0.7, 0.72, 0.78);
const BTN: Color = Color::srgba(0.12, 0.14, 0.22, 0.92);
const BTN_HOVER: Color = Color::srgba(0.28, 0.3, 0.45, 0.95);
const BTN_SEL: Color = Color::srgba(0.45, 0.36, 0.18, 0.95);

fn font(ga: &GameAssets, size: f32) -> TextFont {
    TextFont {
        font: ga.font.clone().into(),
        font_size: FontSize::Px(size),
        ..default()
    }
}

fn shadow() -> TextShadow {
    TextShadow { offset: Vec2::new(1.5, 2.0), color: Color::srgba(0.0, 0.0, 0.0, 0.8) }
}

fn label(ga: &GameAssets, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(s), font(ga, size), TextColor(color), shadow())
}

fn panel_node() -> Node {
    Node {
        padding: UiRect::all(px(12)),
        border: UiRect::all(px(1.5)),
        border_radius: BorderRadius::all(px(10)),
        flex_direction: FlexDirection::Column,
        row_gap: px(6),
        ..default()
    }
}

// ---------------------------------------------------------------- マーカー

#[derive(Component)]
struct HudRoot;
#[derive(Component)]
struct HpFill;
#[derive(Component)]
struct HpText;
#[derive(Component)]
struct StFill;
#[derive(Component)]
struct ExpFill;
#[derive(Component)]
struct LevelText;
#[derive(Component)]
struct ItemsText;
#[derive(Component)]
struct QuestText;
#[derive(Component)]
struct ZoneText;
#[derive(Component)]
struct PromptRoot;
#[derive(Component)]
struct PromptText;
#[derive(Component)]
struct BossBarRoot;
#[derive(Component)]
struct BossFill;
#[derive(Component)]
struct BossName;
#[derive(Component)]
struct BannerRoot;
#[derive(Component)]
struct BannerTitle;
#[derive(Component)]
struct BannerSub;
#[derive(Component)]
struct DialogRoot;
#[derive(Component)]
struct DialogName;
#[derive(Component)]
struct DialogText;
#[derive(Component)]
struct DialogChoices;

/// 会話の選択肢 1 つ (タップで選べるように番号を持つ)。
#[derive(Component)]
struct DialogChoiceItem(usize);

/// 操作説明 (キーボード用。タッチ操作のときは隠す)
#[derive(Component)]
struct KeyHint;
#[derive(Component)]
struct FadeOverlay;
#[derive(Component)]
struct HurtOverlay;
#[derive(Component)]
struct Reticle;
#[derive(Component)]
struct MinimapLayer;
#[derive(Component)]
struct MinimapDots;
#[derive(Component)]
struct MinimapArrow;
#[derive(Component)]
struct EnemyBar {
    target: Entity,
}
#[derive(Component)]
struct EnemyBarFill;
#[derive(Component)]
struct InventoryRoot;
#[derive(Component)]
struct HelpText;

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum MenuAction {
    NewGame,
    Continue,
    Resume,
    ToTitle,
    Respawn,
    KeepPlaying,
    EquipWeapon(usize),
    EquipArmor(usize),
    CloseInventory,
    ToggleHelp,
}

#[derive(Component)]
struct MenuButton {
    action: MenuAction,
    index: usize,
    enabled: bool,
}

#[derive(Resource, Default)]
struct MenuSel(usize);

#[derive(Resource, Default)]
struct BannerQueue {
    queue: VecDeque<Banner>,
    t: f32,
    dur: f32,
    showing: bool,
}

const MAP_SIZE: f32 = 176.0;
const MAP_SCALE: f32 = 1.3;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Banner>()
            .init_resource::<HurtFlash>()
            .init_resource::<MenuSel>()
            .init_resource::<BannerQueue>()
            .add_systems(Startup, spawn_hud)
            .add_systems(OnEnter(GameState::Title), spawn_title)
            .add_systems(OnEnter(GameState::Paused), spawn_pause)
            .add_systems(OnEnter(GameState::GameOver), spawn_gameover)
            .add_systems(OnEnter(GameState::Victory), spawn_victory)
            .add_systems(
                Update,
                (
                    hud_visibility,
                    ui_scale,
                    update_bars,
                    update_prompt,
                    update_boss_bar,
                    banners,
                    dialog_ui,
                    overlays,
                    reticle,
                    minimap_static,
                    minimap_dynamic,
                    enemy_bars,
                    damage_numbers,
                ),
            )
            .add_systems(Update, (menu_buttons, menu_keys, pause_toggle, inventory_toggle, zone_banner, inventory_ui))
            .add_systems(PreUpdate, tap_dialog_choice.after(crate::input::gather_input));
    }
}

// ---------------------------------------------------------------- HUD

fn bar(parent: &mut ChildSpawnerCommands, width: f32, height: f32, color: Color, marker: impl Component) {
    parent
        .spawn((
            Node {
                width: px(width),
                height: px(height),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(height / 2.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.25)),
        ))
        .with_children(|b| {
            b.spawn((
                Node { width: percent(100), height: percent(100), border_radius: BorderRadius::all(px(height / 2.0)), ..default() },
                BackgroundColor(color),
                marker,
            ))
            .with_children(|f| {
                // 上側のハイライト
                f.spawn((
                    Node { width: percent(100), height: percent(40), ..default() },
                    BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.18)),
                ));
            });
        });
}

fn spawn_hud(mut commands: Commands, ga: Res<GameAssets>, controls: Res<Controls>) {
    let ga = &*ga;
    commands
        .spawn((
            Node { width: percent(100), height: percent(100), position_type: PositionType::Absolute, ..default() },
            HudRoot,
            Visibility::Hidden,
            GlobalZIndex(1),
        ))
        .with_children(|root| {
            // 左上: ステータス
            root.spawn((
                Node { position_type: PositionType::Absolute, left: px(18), top: px(16), ..panel_node() },
                BackgroundColor(PANEL),
                BorderColor::all(PANEL_BORDER),
                BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.5), px(0), px(4), px(0), px(12)),
            ))
            .with_children(|p| {
                p.spawn((label(ga, "Lv 1  蒼剣の旅人", 18.0, GOLD), LevelText));
                p.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(8), ..default() })
                    .with_children(|r| {
                        r.spawn(label(ga, "HP", 14.0, DIM));
                        bar(r, 230.0, 16.0, Color::srgb(0.85, 0.22, 0.25), HpFill);
                        r.spawn((label(ga, "100/100", 14.0, TEXT), HpText));
                    });
                p.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(8), ..default() })
                    .with_children(|r| {
                        r.spawn(label(ga, "ST", 14.0, DIM));
                        bar(r, 230.0, 10.0, Color::srgb(0.45, 0.85, 0.35), StFill);
                    });
                p.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(8), ..default() })
                    .with_children(|r| {
                        r.spawn(label(ga, "EX", 14.0, DIM));
                        bar(r, 230.0, 6.0, Color::srgb(0.35, 0.65, 1.0), ExpFill);
                    });
            });

            // 左下: 持ち物
            root.spawn((
                Node { position_type: PositionType::Absolute, left: px(18), bottom: px(16), ..panel_node() },
                BackgroundColor(PANEL),
                BorderColor::all(PANEL_BORDER),
            ))
            .with_children(|p| {
                p.spawn((label(ga, "", 16.0, TEXT), ItemsText));
            });

            // 右上: ミニマップとクエスト
            root.spawn(Node {
                position_type: PositionType::Absolute,
                right: px(18),
                top: px(16),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::End,
                row_gap: px(8),
                ..default()
            })
            .with_children(|col| {
                col.spawn((label(ga, "エルディア平原", 15.0, GOLD), ZoneText));
                col.spawn((
                    Node {
                        width: px(MAP_SIZE),
                        height: px(MAP_SIZE),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(percent(50)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.12, 0.2, 0.12, 0.8)),
                    BorderColor::all(PANEL_BORDER),
                    BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.5), px(0), px(4), px(0), px(12)),
                ))
                .with_children(|m| {
                    m.spawn((Node { position_type: PositionType::Absolute, ..default() }, MinimapLayer));
                    m.spawn((Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() }, MinimapDots));
                    m.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(MAP_SIZE / 2.0 - 7.0),
                            top: px(MAP_SIZE / 2.0 - 9.0),
                            width: px(14),
                            height: px(18),
                            ..default()
                        },
                        MinimapArrow,
                    ))
                    .with_children(|a| {
                        a.spawn((
                            Node { position_type: PositionType::Absolute, left: px(2), top: px(0), width: px(10), height: px(10), ..default() },
                            BackgroundColor(Color::WHITE),
                            UiTransform { rotation: Rot2::degrees(45.0), ..default() },
                        ));
                        a.spawn((
                            Node { position_type: PositionType::Absolute, left: px(3), top: px(4), width: px(8), height: px(12), border_radius: BorderRadius::all(px(3)), ..default() },
                            BackgroundColor(Color::WHITE),
                        ));
                    });
                });
                col.spawn((
                    Node { max_width: px(300), ..panel_node() },
                    BackgroundColor(PANEL),
                    BorderColor::all(PANEL_BORDER),
                ))
                .with_children(|q| {
                    q.spawn(label(ga, "クエスト", 13.0, GOLD));
                    q.spawn((label(ga, "", 15.0, TEXT), QuestText));
                });
            });

            // 下中央: 調べる案内
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    bottom: px(150),
                    width: percent(100),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                PromptRoot,
                Visibility::Hidden,
            ))
            .with_children(|r| {
                r.spawn((
                    Node { padding: UiRect::axes(px(16), px(8)), border: UiRect::all(px(1.5)), border_radius: BorderRadius::all(px(20)), ..default() },
                    BackgroundColor(PANEL),
                    BorderColor::all(PANEL_BORDER),
                ))
                .with_children(|b| {
                    b.spawn((label(ga, "", 17.0, TEXT), PromptText));
                });
            });

            // 上中央: ボスの体力 (タッチのときは上のボタンと左の体力を避ける)
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    top: px(if controls.touch_ui { 60 } else { 22 }),
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(4),
                    ..default()
                },
                BossBarRoot,
                Visibility::Hidden,
            ))
            .with_children(|b| {
                b.spawn((label(ga, "黒騎士ヴァルガ", 20.0, Color::srgb(0.9, 0.75, 1.0)), BossName));
                bar(b, if controls.touch_ui { 330.0 } else { 520.0 }, 14.0, Color::srgb(0.6, 0.2, 0.85), BossFill);
            });

            // 操作説明
            root.spawn((
                Node { position_type: PositionType::Absolute, right: px(18), bottom: px(14), ..default() },
                label(ga, "WASD 移動  クリック/F 攻撃  Shift 回避  Space ジャンプ\nE 調べる  Q 回復薬  R ロックオン  Tab 持ち物  Esc メニュー", 12.0, Color::srgba(1.0, 1.0, 1.0, 0.6)),
                TextLayout { justify: Justify::Right, ..default() },
                KeyHint,
            ));
        });

    // ロックオンの目印
    commands.spawn((
        Node { position_type: PositionType::Absolute, width: px(26), height: px(26), border: UiRect::all(px(3)), border_radius: BorderRadius::all(percent(50)), ..default() },
        BorderColor::all(Color::srgb(1.0, 0.35, 0.3)),
        UiTransform { rotation: Rot2::degrees(45.0), ..default() },
        Visibility::Hidden,
        Reticle,
        GlobalZIndex(2),
    ));

    // バナー
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: percent(20),
                width: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(6),
                ..default()
            },
            BannerRoot,
            Visibility::Hidden,
            GlobalZIndex(5),
        ))
        .with_children(|b| {
            b.spawn((label(ga, "", 40.0, GOLD), BannerTitle));
            b.spawn((label(ga, "", 18.0, TEXT), BannerSub));
        });

    // 会話ウィンドウ
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: px(28),
                width: percent(100),
                justify_content: JustifyContent::Center,
                ..default()
            },
            DialogRoot,
            Visibility::Hidden,
            GlobalZIndex(6),
        ))
        .with_children(|r| {
            r.spawn((
                Node { width: px(760), max_width: percent(94), min_height: px(150), ..panel_node() },
                BackgroundColor(Color::srgba(0.03, 0.04, 0.08, 0.9)),
                BorderColor::all(PANEL_BORDER),
                BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.6), px(0), px(6), px(0), px(18)),
            ))
            .with_children(|p| {
                p.spawn((label(ga, "", 19.0, GOLD), DialogName));
                p.spawn((label(ga, "", 20.0, TEXT), DialogText));
                p.spawn((Node { flex_direction: FlexDirection::Column, row_gap: px(4), margin: UiRect::top(px(6)), ..default() }, DialogChoices));
            });
        });

    // 被弾フラッシュと暗転
    commands.spawn((
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), border: UiRect::all(px(60)), ..default() },
        BorderColor::all(Color::srgba(0.8, 0.0, 0.0, 0.0)),
        HurtOverlay,
        GlobalZIndex(8),
    ));
    commands.spawn((
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        FadeOverlay,
        GlobalZIndex(20),
    ));
}

/// 画面の大きさに合わせて UI 全体を拡大縮小する。
fn ui_scale(windows: Query<&Window>, controls: Res<Controls>, mut scale: ResMut<UiScale>, mut hint: Query<&mut Visibility, With<KeyHint>>) {
    let Ok(w) = windows.single() else { return };
    // スマホは画面が小さいので、PC と同じ割合で縮めると文字が読めない
    let s = if controls.touch_ui {
        (w.height() / 560.0).min(w.width() / 1100.0).clamp(0.6, 1.8)
    } else {
        (w.height() / 820.0).min(w.width() / 1400.0).clamp(0.55, 1.3)
    };
    if controls.touch_ui {
        for mut v in &mut hint {
            *v = Visibility::Hidden;
        }
    }
    if (scale.0 - s).abs() > 0.01 {
        scale.0 = s;
    }
}

fn hud_visibility(state: Res<State<GameState>>, block: Res<UiBlock>, mut q: Query<&mut Visibility, With<HudRoot>>) {
    let show = matches!(state.get(), GameState::Playing) && !block.inventory;
    for mut v in &mut q {
        *v = if show { Visibility::Inherited } else { Visibility::Hidden };
    }
}

#[allow(clippy::type_complexity)]
fn update_bars(
    controls: Res<Controls>,
    stats: Res<PlayerStats>,
    progress: Res<Progress>,
    zone: Res<CurrentZone>,
    mut fills: ParamSet<(
        Query<&mut Node, With<HpFill>>,
        Query<&mut Node, With<StFill>>,
        Query<&mut Node, With<ExpFill>>,
    )>,
    mut texts: ParamSet<(
        Query<&mut Text, With<HpText>>,
        Query<&mut Text, With<LevelText>>,
        Query<&mut Text, With<ItemsText>>,
        Query<&mut Text, With<QuestText>>,
        Query<&mut Text, With<ZoneText>>,
    )>,
) {
    let hp = (stats.hp / stats.max_hp()).clamp(0.0, 1.0);
    let st = (stats.stamina / stats.max_stamina()).clamp(0.0, 1.0);
    let ex = (stats.exp as f32 / stats.exp_to_next() as f32).clamp(0.0, 1.0);
    for mut n in &mut fills.p0() {
        n.width = percent(hp * 100.0);
    }
    for mut n in &mut fills.p1() {
        n.width = percent(st * 100.0);
    }
    for mut n in &mut fills.p2() {
        n.width = percent(ex * 100.0);
    }
    let set = |t: &mut Text, s: String| {
        if t.0 != s {
            t.0 = s;
        }
    };
    for mut t in &mut texts.p0() {
        set(&mut t, format!("{}/{}", stats.hp.ceil() as i32, stats.max_hp() as i32));
    }
    for mut t in &mut texts.p1() {
        set(&mut t, format!("Lv {}  蒼剣の旅人", stats.level));
    }
    for mut t in &mut texts.p2() {
        let key = if controls.touch_ui { "" } else { "  [Q]" };
        set(&mut t, format!("回復薬 x{}{key}      {} G", stats.potions, stats.gold));
    }
    for mut t in &mut texts.p3() {
        set(&mut t, progress.quest_text().iter().map(|s| format!("◆ {s}")).collect::<Vec<_>>().join("\n"));
    }
    for mut t in &mut texts.p4() {
        set(&mut t, zone.0.name().to_string());
    }
}

fn update_prompt(
    near: Res<NearInteract>,
    dialog: Res<Dialog>,
    controls: Res<Controls>,
    mut root: Query<&mut Visibility, With<PromptRoot>>,
    mut text: Query<&mut Text, With<PromptText>>,
) {
    let show = near.0.is_some() && !dialog.active;
    for mut v in &mut root {
        *v = if show { Visibility::Inherited } else { Visibility::Hidden };
    }
    if let Some(s) = &near.0 {
        let key = if controls.touch_ui {
            "話す"
        } else if controls.using_gamepad {
            "Y"
        } else {
            "E"
        };
        for mut t in &mut text {
            let new = format!("[{key}] {s}");
            if t.0 != new {
                t.0 = new;
            }
        }
    }
}

fn update_boss_bar(
    arenas: Query<&BossArena>,
    bosses: Query<&Enemy, With<Boss>>,
    mut root: Query<&mut Visibility, With<BossBarRoot>>,
    mut fill: Query<&mut Node, With<BossFill>>,
    mut name: Query<&mut Text, With<BossName>>,
) {
    let active = arenas.iter().any(|a| a.active);
    let boss = bosses.iter().find(|b| !b.dead || b.state == AiState::Dying);
    let show = active && boss.is_some_and(|b| b.state != AiState::Dying);
    for mut v in &mut root {
        *v = if show { Visibility::Inherited } else { Visibility::Hidden };
    }
    if let Some(b) = boss {
        for mut n in &mut fill {
            n.width = percent(b.hp / b.max_hp * 100.0);
        }
        for mut t in &mut name {
            let s = if b.phase2 { "黒騎士ヴァルガ  -  覚醒" } else { "黒騎士ヴァルガ" };
            if t.0 != s {
                t.0 = s.into();
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn banners(
    time: Res<Time<Real>>,
    mut msgs: MessageReader<Banner>,
    mut q: ResMut<BannerQueue>,
    mut root: Query<(&mut Visibility, &mut Node), With<BannerRoot>>,
    mut title: Query<(&mut Text, &mut TextColor, &mut TextFont), (With<BannerTitle>, Without<BannerSub>)>,
    mut sub: Query<(&mut Text, &mut TextColor), (With<BannerSub>, Without<BannerTitle>)>,
) {
    for m in msgs.read() {
        q.queue.push_back(m.clone());
    }
    let dt = time.delta_secs();
    if !q.showing {
        if let Some(b) = q.queue.pop_front() {
            q.showing = true;
            q.t = 0.0;
            q.dur = if b.big { 3.4 } else { 2.4 };
            for (mut t, _, mut f) in &mut title {
                t.0 = b.title.clone();
                f.font_size = FontSize::Px(if b.big { 42.0 } else { 26.0 });
            }
            for (mut t, _) in &mut sub {
                t.0 = b.sub.clone();
            }
            for (_, mut n) in &mut root {
                n.top = percent(if b.big { 20.0 } else { 26.0 });
            }
        } else {
            for (mut v, _) in &mut root {
                *v = Visibility::Hidden;
            }
            return;
        }
    }
    q.t += dt;
    let u = q.t / q.dur;
    let a = (q.t / 0.3).min(1.0) * ((q.dur - q.t) / 0.5).clamp(0.0, 1.0);
    for (mut v, _) in &mut root {
        *v = Visibility::Inherited;
    }
    for (_, mut c, _) in &mut title {
        c.0 = GOLD.with_alpha(a);
    }
    for (_, mut c) in &mut sub {
        c.0 = TEXT.with_alpha(a);
    }
    if u >= 1.0 {
        q.showing = false;
    }
}

#[allow(clippy::type_complexity)]
fn dialog_ui(
    mut commands: Commands,
    ga: Res<GameAssets>,
    dialog: Res<Dialog>,
    mut root: Query<&mut Visibility, With<DialogRoot>>,
    mut name: Query<&mut Text, (With<DialogName>, Without<DialogText>)>,
    mut text: Query<&mut Text, (With<DialogText>, Without<DialogName>)>,
    choices: Query<Entity, With<DialogChoices>>,
    mut last: Local<(usize, usize, bool, usize)>,
) {
    for mut v in &mut root {
        *v = if dialog.active { Visibility::Inherited } else { Visibility::Hidden };
    }
    if !dialog.active {
        *last = (usize::MAX, 0, false, usize::MAX);
        return;
    }
    for mut t in &mut name {
        if t.0 != dialog.speaker {
            t.0 = dialog.speaker.clone();
        }
    }
    let shown: String = dialog.page_text().chars().take(dialog.shown as usize).collect();
    let more = if dialog.page_done() && (!dialog.at_last() || dialog.choices.is_empty()) { "  ▼" } else { "" };
    for mut t in &mut text {
        let s = format!("{shown}{more}");
        if t.0 != s {
            t.0 = s;
        }
    }
    let showing = dialog.at_last() && dialog.page_done() && !dialog.choices.is_empty();
    let key = (dialog.page, dialog.selected, showing, dialog.choices.len());
    if *last == key {
        return;
    }
    *last = key;
    let Ok(ce) = choices.single() else { return };
    commands.entity(ce).despawn_children();
    if showing {
        commands.entity(ce).with_children(|c| {
            for (i, (s, _)) in dialog.choices.iter().enumerate() {
                let sel = i == dialog.selected;
                c.spawn((
                    Node { padding: UiRect::axes(px(12), px(5)), border_radius: BorderRadius::all(px(6)), ..default() },
                    BackgroundColor(if sel { BTN_SEL } else { Color::srgba(1.0, 1.0, 1.0, 0.05) }),
                    DialogChoiceItem(i),
                ))
                .with_children(|b| {
                    b.spawn(label(&ga, format!("{} {}. {s}", if sel { "▶" } else { "  " }, i + 1), 18.0, if sel { GOLD } else { TEXT }));
                });
            }
        });
    }
}

/// 選択肢が出ているときのタップ: 押した選択肢を選ぶ。選択肢の外のタップでは決定しない。
fn tap_dialog_choice(
    dialog: Res<Dialog>,
    mut controls: ResMut<Controls>,
    items: Query<(&DialogChoiceItem, &ComputedNode, &UiGlobalTransform)>,
) {
    let Some(tap) = controls.tap else { return };
    if !(dialog.active && dialog.at_last() && dialog.page_done() && !dialog.choices.is_empty()) {
        return;
    }
    controls.confirm = false;
    if let Some((item, _, _)) = items.iter().find(|(_, node, tf)| node.contains_point(**tf, tap)) {
        controls.choice = Some(item.0);
    }
}

fn overlays(
    time: Res<Time<Real>>,
    trans: Res<Transition>,
    mut flash: ResMut<HurtFlash>,
    stats: Res<PlayerStats>,
    state: Res<State<GameState>>,
    mut fade: Query<&mut BackgroundColor, With<FadeOverlay>>,
    mut hurt: Query<&mut BorderColor, With<HurtOverlay>>,
) {
    flash.0 = (flash.0 - time.delta_secs() * 2.5).max(0.0);
    let a = fade_alpha(&trans);
    for mut b in &mut fade {
        b.0 = Color::srgba(0.0, 0.0, 0.0, a);
    }
    let low = if *state.get() == GameState::Playing && stats.hp < stats.max_hp() * 0.25 {
        0.18 + (time.elapsed_secs() * 4.0).sin() * 0.08
    } else {
        0.0
    };
    for mut b in &mut hurt {
        *b = BorderColor::all(Color::srgba(0.75, 0.0, 0.02, (flash.0 * 0.45).max(low)));
    }
}

#[allow(clippy::type_complexity)]
fn reticle(
    player: Query<&PlayerCtl, With<Player>>,
    enemies: Query<(&GlobalTransform, &Enemy)>,
    cam: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut q: Query<(&mut Node, &mut Visibility), With<Reticle>>,
    state: Res<State<GameState>>,
    ui_scale: Res<UiScale>,
) {
    let Ok((mut n, mut v)) = q.single_mut() else { return };
    *v = Visibility::Hidden;
    if *state.get() != GameState::Playing {
        return;
    }
    let Ok(ctl) = player.single() else { return };
    let Some((et, en)) = ctl.lock.and_then(|e| enemies.get(e).ok()) else { return };
    let Ok((c, ct)) = cam.single() else { return };
    let p = et.translation() + Vec3::Y * en.kind.def().bar_height * 0.55;
    if let Ok(s) = c.world_to_viewport(ct, p) {
        let s = s / ui_scale.0;
        n.left = px(s.x - 13.0);
        n.top = px(s.y - 13.0);
        *v = Visibility::Inherited;
    }
}

// ---------------------------------------------------------------- ミニマップ

fn map_pos(world: Vec2, origin: Vec2) -> Vec2 {
    (world - origin) * MAP_SCALE
}

fn minimap_static(
    mut commands: Commands,
    mut msgs: MessageReader<ZoneEntered>,
    layer: Query<Entity, With<MinimapLayer>>,
    colliders: Res<Colliders>,
) {
    let Some(z) = msgs.read().last().map(|m| m.0) else { return };
    let Ok(le) = layer.single() else { return };
    commands.entity(le).despawn_children();
    let dot = |c: &mut ChildSpawnerCommands, p: Vec2, size: Vec2, color: Color, round: bool| {
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(p.x - size.x / 2.0),
                top: px(p.y - size.y / 2.0),
                width: px(size.x),
                height: px(size.y),
                border_radius: if round { BorderRadius::all(percent(50)) } else { BorderRadius::all(px(1)) },
                ..default()
            },
            BackgroundColor(color),
        ));
    };
    match z {
        ZoneId::Field => {
            let origin = Vec2::splat(-terrain::HALF);
            commands.entity(le).with_children(|c| {
                // 外周の山
                dot(c, map_pos(Vec2::ZERO, origin), Vec2::splat(terrain::WORLD_RADIUS * 2.0 * MAP_SCALE), Color::srgba(0.3, 0.45, 0.22, 0.9), true);
                for path in terrain::PATHS {
                    for w in path.windows(2) {
                        let n = (w[0].distance(w[1]) / 3.0) as i32;
                        for i in 0..=n {
                            let p = w[0].lerp(w[1], i as f32 / n.max(1) as f32);
                            dot(c, map_pos(p, origin), Vec2::splat(5.0), Color::srgb(0.6, 0.5, 0.35), true);
                        }
                    }
                }
                dot(c, map_pos(terrain::LAKE, origin), Vec2::splat(40.0 * MAP_SCALE), Color::srgb(0.2, 0.45, 0.7), true);
                dot(c, map_pos(Vec2::ZERO, origin), Vec2::splat(30.0 * MAP_SCALE), Color::srgba(0.7, 0.6, 0.45, 0.8), true);
                dot(c, map_pos(terrain::RUINS, origin), Vec2::splat(22.0 * MAP_SCALE), Color::srgba(0.55, 0.5, 0.6, 0.9), true);
            });
        }
        ZoneId::Dungeon => {
            let origin = Vec2::ZERO;
            if let Some(g) = &colliders.grid {
                commands.entity(le).with_children(|c| {
                    for z in 0..g.h {
                        for x in 0..g.w {
                            if !g.is_wall(x, z) {
                                let p = Vec2::new(x as f32 + 0.5, z as f32 + 0.5) * g.size;
                                dot(c, map_pos(p, origin), Vec2::splat(g.size * MAP_SCALE + 0.5), Color::srgba(0.45, 0.42, 0.5, 0.9), false);
                            }
                        }
                    }
                });
            }
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn minimap_dynamic(
    mut commands: Commands,
    zone: Res<CurrentZone>,
    player: Query<(&Transform, &PlayerCtl), With<Player>>,
    enemies: Query<(&Transform, &Enemy), Without<Player>>,
    things: Query<(&GlobalTransform, &Interactable)>,
    npcs: Query<&Transform, (With<Npc>, Without<Player>)>,
    mut layer: Query<&mut Node, (With<MinimapLayer>, Without<MinimapArrow>)>,
    dots: Query<Entity, With<MinimapDots>>,
    mut arrow: Query<&mut UiTransform, With<MinimapArrow>>,
    mut acc: Local<f32>,
    time: Res<Time<Real>>,
) {
    let Ok((pt, ctl)) = player.single() else { return };
    let origin = if zone.0 == ZoneId::Field { Vec2::splat(-terrain::HALF) } else { Vec2::ZERO };
    let pp = pt.translation.xz();
    let center = Vec2::splat(MAP_SIZE / 2.0);
    for mut n in &mut layer {
        let off = center - map_pos(pp, origin);
        n.left = px(off.x);
        n.top = px(off.y);
    }
    for mut a in &mut arrow {
        a.rotation = Rot2::radians(-ctl.facing);
    }
    // 動くものの点は 0.1 秒ごとに作り直す
    *acc += time.delta_secs();
    if *acc < 0.1 {
        return;
    }
    *acc = 0.0;
    let Ok(de) = dots.single() else { return };
    commands.entity(de).despawn_children();
    let range = MAP_SIZE / 2.0 / MAP_SCALE;
    let mut list: Vec<(Vec2, Color, f32)> = Vec::new();
    for (t, e) in &enemies {
        if !e.dead {
            let c = if e.kind == EnemyKind::Boss { Color::srgb(0.8, 0.3, 1.0) } else { Color::srgb(1.0, 0.3, 0.25) };
            list.push((t.translation.xz(), c, if e.kind == EnemyKind::Boss { 10.0 } else { 6.0 }));
        }
    }
    for t in &npcs {
        list.push((t.translation.xz(), Color::srgb(1.0, 0.85, 0.3), 7.0));
    }
    for (g, it) in &things {
        let c = match it.kind {
            InteractKind::Portal | InteractKind::ReturnPortal => Color::srgb(0.7, 0.4, 1.0),
            InteractKind::SaveCrystal => Color::srgb(0.4, 0.8, 1.0),
            InteractKind::Chest(_) => Color::srgb(1.0, 0.75, 0.2),
            InteractKind::Npc(_) => continue,
        };
        list.push((g.translation().xz(), c, 8.0));
    }
    commands.entity(de).with_children(|c| {
        for (p, color, s) in list {
            let d = p - pp;
            if d.length() > range {
                continue;
            }
            let m = center + d * MAP_SCALE;
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(m.x - s / 2.0),
                    top: px(m.y - s / 2.0),
                    width: px(s),
                    height: px(s),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(percent(50)),
                    ..default()
                },
                BackgroundColor(color),
                BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            ));
        }
    });
}

// ---------------------------------------------------------------- 敵の体力バーとダメージ数字

#[allow(clippy::type_complexity)]
fn enemy_bars(
    mut commands: Commands,
    enemies: Query<(Entity, &GlobalTransform, &Enemy)>,
    player: Query<&PlayerCtl, With<Player>>,
    cam: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut bars: Query<(Entity, &EnemyBar, &mut Node, &mut Visibility), Without<EnemyBarFill>>,
    mut fills: Query<(&ChildOf, &mut Node), With<EnemyBarFill>>,
    mut map: Local<HashMap<Entity, Entity>>,
    state: Res<State<GameState>>,
    ui_scale: Res<UiScale>,
) {
    let Ok((c, ct)) = cam.single() else { return };
    let lock = player.single().ok().and_then(|p| p.lock);
    let playing = *state.get() == GameState::Playing;
    // 新しく戦闘に入った敵にバーを付ける
    for (e, _, en) in &enemies {
        if en.kind == EnemyKind::Boss {
            continue;
        }
        if (en.last_hit < 6.0 || lock == Some(e)) && !en.dead && !map.contains_key(&e) {
            let bar = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: px(64),
                        height: px(7),
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(3)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                    BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.3)),
                    EnemyBar { target: e },
                    Visibility::Hidden,
                    GlobalZIndex(0),
                ))
                .with_children(|b| {
                    b.spawn((Node { width: percent(100), height: percent(100), ..default() }, BackgroundColor(Color::srgb(0.9, 0.25, 0.2)), EnemyBarFill));
                })
                .id();
            map.insert(e, bar);
        }
    }
    for (be, bar, mut node, mut vis) in &mut bars {
        let Ok((e, gt, en)) = enemies.get(bar.target) else {
            commands.entity(be).despawn();
            map.remove(&bar.target);
            continue;
        };
        if en.dead || (en.last_hit > 8.0 && lock != Some(e)) {
            commands.entity(be).despawn();
            map.remove(&e);
            continue;
        }
        let p = gt.translation() + Vec3::Y * en.kind.def().bar_height;
        match c.world_to_viewport(ct, p) {
            Ok(s) if playing => {
                let s = s / ui_scale.0;
                node.left = px(s.x - 32.0);
                node.top = px(s.y);
                *vis = Visibility::Inherited;
            }
            _ => *vis = Visibility::Hidden,
        }
        for (parent, mut fnode) in &mut fills {
            if parent.parent() == be {
                fnode.width = percent((en.hp / en.max_hp * 100.0).max(0.0));
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn damage_numbers(
    mut commands: Commands,
    time: Res<Time<Real>>,
    ga: Res<GameAssets>,
    cam: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    added: Query<(Entity, &DamageNumber), (Added<DamageNumber>, Without<Node>)>,
    mut q: Query<(Entity, &mut DamageNumber, &mut Node, &mut TextColor)>,
    ui_scale: Res<UiScale>,
) {
    for (e, d) in &added {
        commands.entity(e).insert((
            Node { position_type: PositionType::Absolute, ..default() },
            Text::new(d.text.clone()),
            font(&ga, d.size),
            TextColor(d.color),
            TextShadow { offset: Vec2::new(2.0, 2.0), color: Color::srgba(0.0, 0.0, 0.0, 0.9) },
            GlobalZIndex(3),
        ));
    }
    let Ok((c, ct)) = cam.single() else { return };
    let dt = time.delta_secs();
    for (e, mut d, mut node, mut color) in &mut q {
        d.t += dt;
        if d.t > 1.0 {
            commands.entity(e).despawn();
            continue;
        }
        let rise = d.t * 1.3;
        if let Ok(s) = c.world_to_viewport(ct, d.world + Vec3::Y * rise) {
            let s = s / ui_scale.0;
            node.left = px(s.x - 20.0 + d.drift * d.t);
            node.top = px(s.y - 30.0 * (1.0 - (1.0 - d.t).powi(3)));
        }
        let a = if d.t < 0.7 { 1.0 } else { 1.0 - (d.t - 0.7) / 0.3 };
        color.0 = d.color.with_alpha(a);
    }
}

// ---------------------------------------------------------------- メニュー

fn button(parent: &mut ChildSpawnerCommands, ga: &GameAssets, text: &str, action: MenuAction, index: usize, enabled: bool) {
    parent
        .spawn((
            Button,
            Node {
                width: px(300),
                padding: UiRect::axes(px(18), px(11)),
                justify_content: JustifyContent::Center,
                border: UiRect::all(px(1.5)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(BTN),
            BorderColor::all(PANEL_BORDER),
            MenuButton { action, index, enabled },
        ))
        .with_children(|b| {
            b.spawn(label(ga, text, 21.0, if enabled { TEXT } else { Color::srgba(0.6, 0.6, 0.6, 0.6) }));
        });
}

fn fullscreen(color: Color) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: px(12),
            ..default()
        },
        BackgroundColor(color),
        GlobalZIndex(10),
    )
}

fn spawn_title(mut commands: Commands, ga: Res<GameAssets>, mut sel: ResMut<MenuSel>, controls: Res<Controls>) {
    let ga = &*ga;
    let has = save::has_save();
    sel.0 = if has { 1 } else { 0 };
    commands
        .spawn((fullscreen(Color::srgba(0.0, 0.02, 0.06, 0.28)), DespawnOnExit(GameState::Title)))
        .with_children(|r| {
            // スマホの横画面は高さが足りないので題字を小さくする
            let big = if controls.touch_ui { 60.0 } else { 84.0 };
            r.spawn(label(ga, "ETERNAL BLADE OF", 18.0, Color::srgba(1.0, 0.95, 0.85, 0.85)));
            r.spawn((
                Text::new("蒼剣のエルディア"),
                font(ga, big),
                TextColor(Color::srgb(0.98, 0.9, 0.7)),
                TextShadow { offset: Vec2::new(3.0, 4.0), color: Color::srgba(0.05, 0.1, 0.3, 0.9) },
            ));
            r.spawn(label(ga, "〜 目覚めし黒騎士 〜", 22.0, Color::srgb(0.7, 0.85, 1.0)));
            r.spawn(Node { height: px(14), ..default() });
            button(r, ga, "はじめから", MenuAction::NewGame, 0, true);
            button(r, ga, "つづきから", MenuAction::Continue, 1, has);
            button(r, ga, "操作方法", MenuAction::ToggleHelp, 2, true);
            r.spawn((
                Node { margin: UiRect::top(px(10)), ..panel_node() },
                BackgroundColor(PANEL),
                BorderColor::all(PANEL_BORDER),
                Visibility::Hidden,
                HelpText,
            ))
            .with_children(|p| {
                let keys = [
                    "WASD / 左スティック ― 移動",
                    "マウス / 右スティック ― カメラ (画面クリックでマウス固定)",
                    "左クリック / F / X ― 攻撃 (連打で 3 段コンボ)",
                    "Shift / 右クリック / B ― 回避 (無敵時間あり)",
                    "Space / A ― ジャンプ     E / Y ― 話す・調べる",
                    "Q / RT ― 回復薬     R / LT ― ロックオン",
                    "Tab / Back ― 持ち物・装備     Esc / Start ― メニュー",
                ];
                let touch = [
                    "画面の左をなぞる ― 移動     右をなぞる ― カメラ",
                    "攻撃 ― 連打で 3 段コンボ     回避 ― 無敵時間あり",
                    "話す ― 話す・調べる     ロック ― ロックオン",
                    "会話はタップで送る。選択肢もタップで選ぶ",
                ];
                let lines: &[&str] = if controls.touch_ui { &touch } else { &keys };
                for &line in lines {
                    p.spawn(label(ga, line, 16.0, TEXT));
                }
            });
            let start = if controls.touch_ui { "ボタンをタップして開始" } else { "Enter / クリックで開始" };
            r.spawn((
                Node { position_type: PositionType::Absolute, bottom: px(14), ..default() },
                label(ga, format!("Rust + Bevy + WebAssembly   ―   {start}"), 13.0, Color::srgba(1.0, 1.0, 1.0, 0.6)),
            ));
        });
}

fn spawn_pause(mut commands: Commands, ga: Res<GameAssets>, mut sel: ResMut<MenuSel>, stats: Res<PlayerStats>) {
    let ga = &*ga;
    sel.0 = 0;
    commands
        .spawn((fullscreen(Color::srgba(0.0, 0.0, 0.0, 0.55)), DespawnOnExit(GameState::Paused)))
        .with_children(|r| {
            r.spawn(label(ga, "ポーズ", 48.0, GOLD));
            r.spawn(label(ga, format!("プレイ時間 {}", fmt_time(stats.play_time)), 16.0, DIM));
            r.spawn(Node { height: px(16), ..default() });
            button(r, ga, "ゲームに戻る", MenuAction::Resume, 0, true);
            button(r, ga, "タイトルへ戻る", MenuAction::ToTitle, 1, true);
            r.spawn(Node { height: px(10), ..default() });
            r.spawn(label(ga, "※ 記録は村の青いクリスタルで行えます", 14.0, DIM));
        });
}

fn spawn_gameover(mut commands: Commands, ga: Res<GameAssets>, mut sel: ResMut<MenuSel>) {
    let ga = &*ga;
    sel.0 = 0;
    commands
        .spawn((fullscreen(Color::srgba(0.12, 0.0, 0.0, 0.6)), DespawnOnExit(GameState::GameOver)))
        .with_children(|r| {
            r.spawn((Text::new("力尽きた…"), font(ga, 64.0), TextColor(Color::srgb(0.95, 0.3, 0.3)), shadow()));
            r.spawn(label(ga, "所持金の 2 割を失い、村で目を覚ます", 18.0, TEXT));
            r.spawn(Node { height: px(20), ..default() });
            button(r, ga, "村で目を覚ます", MenuAction::Respawn, 0, true);
            button(r, ga, "タイトルへ戻る", MenuAction::ToTitle, 1, true);
        });
}

fn fmt_time(t: f32) -> String {
    let s = t as u32;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

fn spawn_victory(mut commands: Commands, ga: Res<GameAssets>, mut sel: ResMut<MenuSel>, stats: Res<PlayerStats>, mut music: ResMut<Music>, progress: Res<Progress>) {
    let ga = &*ga;
    sel.0 = 0;
    *music = Music::Victory;
    save::write_save(&stats, &progress);
    commands
        .spawn((fullscreen(Color::srgba(0.02, 0.03, 0.08, 0.7)), DespawnOnExit(GameState::Victory)))
        .with_children(|r| {
            r.spawn(label(ga, "THE END", 20.0, DIM));
            r.spawn((
                Text::new("エルディアに平和が戻った"),
                font(ga, 56.0),
                TextColor(Color::srgb(0.98, 0.88, 0.6)),
                TextShadow { offset: Vec2::new(3.0, 4.0), color: Color::srgba(0.0, 0.0, 0.0, 0.9) },
            ));
            r.spawn(label(ga, "黒騎士ヴァルガは倒れ、古代遺跡に再び静寂が訪れた。", 18.0, TEXT));
            r.spawn((
                Node { margin: UiRect::vertical(px(16)), ..panel_node() },
                BackgroundColor(PANEL),
                BorderColor::all(PANEL_BORDER),
            ))
            .with_children(|p| {
                p.spawn(label(ga, format!("プレイ時間   {}", fmt_time(stats.play_time)), 18.0, TEXT));
                p.spawn(label(ga, format!("レベル       {}", stats.level), 18.0, TEXT));
                p.spawn(label(ga, format!("倒した魔物   {}", stats.kills), 18.0, TEXT));
                p.spawn(label(ga, format!("所持金       {} G", stats.gold), 18.0, TEXT));
            });
            button(r, ga, "冒険を続ける", MenuAction::KeepPlaying, 0, true);
            button(r, ga, "タイトルへ戻る", MenuAction::ToTitle, 1, true);
            r.spawn(label(ga, "遊んでくれてありがとう!", 15.0, DIM));
        });
}

#[derive(SystemParam)]
struct MenuCtx<'w, 's> {
    next: ResMut<'w, NextState<GameState>>,
    stats: ResMut<'w, PlayerStats>,
    progress: ResMut<'w, Progress>,
    zreq: MessageWriter<'w, ZoneRequest>,
    music: ResMut<'w, Music>,
    player: Query<'w, 's, &'static mut PlayerCtl, With<Player>>,
    block: ResMut<'w, UiBlock>,
    rig: ResMut<'w, CamRig>,
    help: Query<'w, 's, &'static mut Visibility, With<HelpText>>,
    banner: MessageWriter<'w, Banner>,
    sfx: MessageWriter<'w, PlaySfx>,
}

impl MenuCtx<'_, '_> {
    fn respawn(&mut self) {
        self.zreq.write(ZoneRequest { zone: ZoneId::Field, spawn: VILLAGE_SPAWN, facing: 0.0 });
        for mut c in self.player.iter_mut() {
            revive(&mut c);
            c.facing = 0.0;
        }
        self.rig.yaw = 0.0;
        self.rig.pitch = 0.35;
        self.rig.focus = Vec3::ZERO;
    }

    fn run(&mut self, a: MenuAction) {
        match a {
            MenuAction::NewGame => {
                *self.stats = PlayerStats::default();
                *self.progress = Progress::default();
                self.respawn();
                *self.music = Music::Field;
                self.next.set(GameState::Playing);
                self.banner.write(Banner::big("エルディア平原", "まずは村の長老に話を聞こう"));
            }
            MenuAction::Continue => {
                if let Some((s, p)) = save::read_save() {
                    *self.stats = s;
                    *self.progress = p;
                    self.respawn();
                    *self.music = Music::Field;
                    self.next.set(GameState::Playing);
                    self.banner.write(Banner::big("エルディア平原", "旅の記録を読み込んだ"));
                }
            }
            MenuAction::Resume | MenuAction::KeepPlaying => {
                if a == MenuAction::KeepPlaying {
                    *self.music = Music::Field;
                }
                self.next.set(GameState::Playing);
            }
            MenuAction::ToTitle => {
                *self.music = Music::None;
                self.respawn();
                self.block.inventory = false;
                self.block.dialog = false;
                self.block.shop = false;
                self.next.set(GameState::Title);
            }
            MenuAction::Respawn => {
                let g = self.stats.gold / 5;
                self.stats.gold -= g;
                self.stats.hp = self.stats.max_hp();
                self.stats.stamina = self.stats.max_stamina();
                self.respawn();
                *self.music = Music::Field;
                self.next.set(GameState::Playing);
            }
            MenuAction::EquipWeapon(i) => {
                if self.stats.weapons[i] {
                    self.stats.weapon = i;
                }
            }
            MenuAction::EquipArmor(i) => {
                if self.stats.armors[i] {
                    let ratio = self.stats.hp / self.stats.max_hp();
                    self.stats.armor = i;
                    self.stats.hp = (self.stats.max_hp() * ratio).max(1.0);
                }
            }
            MenuAction::CloseInventory => self.block.inventory = false,
            MenuAction::ToggleHelp => {
                for mut v in self.help.iter_mut() {
                    *v = if *v == Visibility::Hidden { Visibility::Inherited } else { Visibility::Hidden };
                }
            }
        }
    }
}

fn menu_buttons(
    mut q: Query<(&Interaction, &MenuButton, &mut BackgroundColor), Changed<Interaction>>,
    mut ctx: MenuCtx,
    mut sel: ResMut<MenuSel>,
) {
    for (i, b, mut bg) in &mut q {
        if !b.enabled {
            continue;
        }
        match *i {
            Interaction::Pressed => {
                ctx.sfx.write(PlaySfx(SfxKind::Select));
                ctx.run(b.action);
            }
            Interaction::Hovered => {
                bg.0 = BTN_HOVER;
                sel.0 = b.index;
            }
            Interaction::None => bg.0 = BTN,
        }
    }
}

fn menu_keys(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut sel: ResMut<MenuSel>,
    mut buttons: Query<(&MenuButton, &mut BackgroundColor, &Interaction)>,
    gamepads: Query<&Gamepad>,
    mut ctx: MenuCtx,
) {
    if *state.get() == GameState::Playing && !ctx.block.inventory {
        return;
    }
    let is_equip = |a: MenuAction| matches!(a, MenuAction::EquipArmor(_) | MenuAction::EquipWeapon(_));
    let menu: Vec<(usize, MenuAction, bool)> = buttons
        .iter()
        .filter(|(b, _, _)| !is_equip(b.action))
        .map(|(b, _, _)| (b.index, b.action, b.enabled))
        .collect();
    if menu.is_empty() {
        return;
    }
    let n = menu.iter().map(|m| m.0).max().unwrap_or(0) + 1;
    let pad_down = gamepads.iter().any(|g| g.just_pressed(GamepadButton::DPadDown));
    let pad_up = gamepads.iter().any(|g| g.just_pressed(GamepadButton::DPadUp));
    let pad_ok = gamepads.iter().any(|g| g.just_pressed(GamepadButton::South));
    let mut moved = false;
    if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) || pad_down {
        sel.0 = (sel.0 + 1) % n;
        moved = true;
    }
    if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) || pad_up {
        sel.0 = (sel.0 + n - 1) % n;
        moved = true;
    }
    if moved {
        ctx.sfx.write(PlaySfx(SfxKind::Select));
    }
    for (b, mut bg, i) in &mut buttons {
        if is_equip(b.action) {
            continue;
        }
        if *i == Interaction::None {
            bg.0 = if b.index == sel.0 { BTN_SEL } else { BTN };
        }
    }
    let ok = keys.just_pressed(KeyCode::Enter) || pad_ok || (keys.just_pressed(KeyCode::Space) && *state.get() != GameState::Playing);
    if ok
        && let Some((_, a, en)) = menu.iter().find(|m| m.0 == sel.0)
        && *en
    {
        ctx.sfx.write(PlaySfx(SfxKind::Select));
        ctx.run(*a);
    }
}

fn pause_toggle(
    controls: Res<Controls>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    mut block: ResMut<UiBlock>,
    dialog: Res<Dialog>,
    player: Query<&PlayerCtl, With<Player>>,
    #[cfg(target_arch = "wasm32")] mut was_locked: Local<bool>,
) {
    let _ = &player;
    match state.get() {
        GameState::Playing => {
            // ブラウザ側で Esc によりポインタロックが外れたら一時停止する
            #[cfg(target_arch = "wasm32")]
            {
                let locked = crate::input::pointer_locked_now();
                if *was_locked && !locked && !block.any() {
                    *was_locked = false;
                    next.set(GameState::Paused);
                    return;
                }
                *was_locked = locked;
            }
            if controls.pause {
                if block.inventory {
                    block.inventory = false;
                } else if !dialog.active {
                    next.set(GameState::Paused);
                }
            }
        }
        GameState::Paused
            if controls.pause => {
                next.set(GameState::Playing);
            }
        _ => {}
    }
}

fn inventory_toggle(
    controls: Res<Controls>,
    state: Res<State<GameState>>,
    mut block: ResMut<UiBlock>,
    dialog: Res<Dialog>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if *state.get() != GameState::Playing || dialog.active {
        return;
    }
    if controls.inventory {
        block.inventory = !block.inventory;
        sfx.write(PlaySfx(SfxKind::Select));
    }
}

fn zone_banner(mut msgs: MessageReader<ZoneEntered>, state: Res<State<GameState>>, trans: Res<Transition>, mut banner: MessageWriter<Banner>) {
    for m in msgs.read() {
        // ポータルで移動したときだけ表示 (メニューからの開始時は別のバナーを出す)
        if *state.get() == GameState::Playing && trans.pending.is_some() {
            let sub = match m.0 {
                ZoneId::Field => "",
                ZoneId::Dungeon => "最深部に黒騎士が待ち受ける",
            };
            banner.write(Banner::big(m.0.name(), sub));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn inventory_ui(
    mut commands: Commands,
    ga: Res<GameAssets>,
    block: Res<UiBlock>,
    stats: Res<PlayerStats>,
    controls: Res<Controls>,
    roots: Query<Entity, With<InventoryRoot>>,
    mut last: Local<Option<(usize, usize, u32, u32)>>,
) {
    if !block.inventory {
        for e in &roots {
            commands.entity(e).despawn();
        }
        *last = None;
        return;
    }
    let key = (stats.weapon, stats.armor, stats.gold, stats.level);
    if *last == Some(key) && !roots.is_empty() {
        return;
    }
    *last = Some(key);
    for e in &roots {
        commands.entity(e).despawn();
    }
    let ga = &*ga;
    commands
        .spawn((fullscreen(Color::srgba(0.0, 0.0, 0.03, 0.62)), InventoryRoot))
        .with_children(|r| {
            r.spawn(label(ga, "持ち物・装備", 34.0, GOLD));
            r.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(18), ..default() })
                .with_children(|row| {
                    // ステータス
                    row.spawn((Node { width: px(270), ..panel_node() }, BackgroundColor(PANEL), BorderColor::all(PANEL_BORDER)))
                        .with_children(|p| {
                            p.spawn(label(ga, "ステータス", 16.0, GOLD));
                            for (k, v) in [
                                ("レベル", format!("{}", stats.level)),
                                ("HP", format!("{} / {}", stats.hp.ceil(), stats.max_hp())),
                                ("攻撃力", format!("{}", stats.atk())),
                                ("防御力", format!("{}", stats.def())),
                                ("次のレベルまで", format!("{} EXP", stats.exp_to_next() - stats.exp)),
                                ("所持金", format!("{} G", stats.gold)),
                                ("回復薬", format!("{} 個", stats.potions)),
                                ("倒した魔物", format!("{}", stats.kills)),
                                ("プレイ時間", fmt_time(stats.play_time)),
                            ] {
                                p.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|l| {
                                    l.spawn(label(ga, k, 16.0, DIM));
                                    l.spawn(label(ga, v, 16.0, TEXT));
                                });
                            }
                        });
                    // 装備
                    for (title, list, owned, cur, is_weapon) in [
                        ("武器", WEAPONS, &stats.weapons, stats.weapon, true),
                        ("防具", ARMORS, &stats.armors, stats.armor, false),
                    ] {
                        row.spawn((Node { width: px(300), ..panel_node() }, BackgroundColor(PANEL), BorderColor::all(PANEL_BORDER)))
                            .with_children(|p| {
                                p.spawn(label(ga, title, 16.0, GOLD));
                                for (i, eq) in list.iter().enumerate() {
                                    if !owned[i] {
                                        p.spawn(label(ga, "？？？", 15.0, Color::srgba(0.6, 0.6, 0.6, 0.5)));
                                        continue;
                                    }
                                    let equipped = i == cur;
                                    let mut stat = Vec::new();
                                    if eq.atk > 0 {
                                        stat.push(format!("攻+{}", eq.atk));
                                    }
                                    if eq.def > 0 {
                                        stat.push(format!("防+{}", eq.def));
                                    }
                                    if eq.hp > 0 {
                                        stat.push(format!("HP+{}", eq.hp));
                                    }
                                    let action = if is_weapon { MenuAction::EquipWeapon(i) } else { MenuAction::EquipArmor(i) };
                                    p.spawn((
                                        Button,
                                        Node {
                                            flex_direction: FlexDirection::Column,
                                            padding: UiRect::axes(px(10), px(6)),
                                            border: UiRect::all(px(1.5)),
                                            border_radius: BorderRadius::all(px(6)),
                                            ..default()
                                        },
                                        BackgroundColor(if equipped { BTN_SEL } else { BTN }),
                                        BorderColor::all(if equipped { GOLD } else { Color::srgba(1.0, 1.0, 1.0, 0.15) }),
                                        MenuButton { action, index: 100 + i, enabled: true },
                                    ))
                                    .with_children(|b| {
                                        b.spawn(label(ga, format!("{}{}", if equipped { "E  " } else { "" }, eq.name), 17.0, if equipped { GOLD } else { TEXT }));
                                        b.spawn(label(ga, format!("{}  {}", eq.desc, stat.join(" ")), 13.0, DIM));
                                    });
                                }
                            });
                    }
                });
            button(r, ga, if controls.touch_ui { "閉じる" } else { "閉じる (Tab)" }, MenuAction::CloseInventory, 0, true);
            let _ = dungeon::TILE;
        });
}
