//! キーボード・マウス・ゲームパッド・タッチ入力を 1 つの `Controls` にまとめる。
//!
//! タッチ操作の画面 (仮想スティックとボタン) は touch.js が HTML で重ねて表示し、
//! `window.__touchTake()` で前のフレームからの入力を渡してくる。

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::{GameState, UiBlock, npc::NearInteract};

#[derive(Resource, Default, Debug)]
pub struct Controls {
    /// x: 右, y: 前
    pub movement: Vec2,
    /// カメラ回転 (ラジアン換算済み)
    pub look: Vec2,
    pub zoom: f32,
    pub attack: bool,
    pub jump: bool,
    pub roll: bool,
    pub interact: bool,
    pub potion: bool,
    pub lock_on: bool,
    pub pause: bool,
    pub inventory: bool,
    /// 会話送り・決定
    pub confirm: bool,
    pub choice: Option<usize>,
    pub using_gamepad: bool,
    /// タッチ操作の画面を出しているか (スマホ・タブレット)
    pub touch_ui: bool,
    /// ゲーム画面を短くタップした位置 (UI と同じピクセル座標)
    pub tap: Option<Vec2>,
}

// touch.js のボタン番号
const TOUCH_ATTACK: u32 = 1 << 0;
const TOUCH_ROLL: u32 = 1 << 1;
const TOUCH_JUMP: u32 = 1 << 2;
const TOUCH_TALK: u32 = 1 << 3;
const TOUCH_POTION: u32 = 1 << 4;
const TOUCH_LOCK: u32 = 1 << 5;
const TOUCH_BAG: u32 = 1 << 6;
const TOUCH_MENU: u32 = 1 << 7;

/// touch.js から受け取った 1 フレーム分のタッチ入力。
#[derive(Default)]
struct TouchFrame {
    stick: Vec2,
    look: Vec2,
    buttons: u32,
    tap: Option<Vec2>,
}

#[cfg(target_arch = "wasm32")]
fn touch_ui_enabled() -> bool {
    web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w, &"__touchUI".into()).ok())
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

#[cfg(not(target_arch = "wasm32"))]
fn touch_ui_enabled() -> bool {
    false
}

#[cfg(target_arch = "wasm32")]
fn take_touch() -> Option<TouchFrame> {
    use wasm_bindgen::JsCast;
    let w = web_sys::window()?;
    let f = js_sys::Reflect::get(&w, &"__touchTake".into()).ok()?.dyn_into::<js_sys::Function>().ok()?;
    let a = js_sys::Array::from(&f.call0(&w).ok()?);
    let n = |i: u32| a.get(i).as_f64().unwrap_or(0.0) as f32;
    Some(TouchFrame {
        stick: Vec2::new(n(0), n(1)),
        look: Vec2::new(n(2), n(3)),
        buttons: n(4) as u32,
        tap: (n(5) > 0.0).then(|| Vec2::new(n(6), n(7))),
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn take_touch() -> Option<TouchFrame> {
    None
}

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Controls { touch_ui: touch_ui_enabled(), ..default() })
            .add_systems(PreUpdate, gather_input.after(bevy::input::InputSystems))
            .add_systems(Update, (cursor_lock, page_mode));
    }
}

/// ブラウザ上で実際にポインタロック中か。
#[cfg(target_arch = "wasm32")]
pub fn pointer_locked_now() -> bool {
    web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.pointer_lock_element())
        .is_some()
}

fn is_locked(cursor: &CursorOptions) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = cursor;
        pointer_locked_now()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        cursor.grab_mode != CursorGrabMode::None
    }
}

pub fn gather_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    gamepads: Query<&Gamepad>,
    cursor: Single<&CursorOptions>,
    mut c: ResMut<Controls>,
) {
    let locked = is_locked(&cursor);
    let mut mv = Vec2::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        mv.y += 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        mv.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        mv.x += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        mv.x -= 1.0;
    }
    let mut look = Vec2::ZERO;
    if locked {
        look += motion.delta * 0.0032;
    }
    // マウスを使わない人向けのカメラ回転
    if keys.pressed(KeyCode::KeyJ) {
        look.x -= 0.035;
    }
    if keys.pressed(KeyCode::KeyL) {
        look.x += 0.035;
    }
    if keys.pressed(KeyCode::KeyI) {
        look.y -= 0.025;
    }
    if keys.pressed(KeyCode::KeyK) {
        look.y += 0.025;
    }

    let digit = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
    ];
    let mut choice = digit.iter().position(|k| keys.just_pressed(*k));

    let mut attack = (locked && mouse.just_pressed(MouseButton::Left))
        || keys.just_pressed(KeyCode::KeyF);
    let mut jump = keys.just_pressed(KeyCode::Space);
    let mut roll = keys.just_pressed(KeyCode::ShiftLeft)
        || keys.just_pressed(KeyCode::ShiftRight)
        || (locked && mouse.just_pressed(MouseButton::Right));
    let mut interact = keys.just_pressed(KeyCode::KeyE);
    let mut potion = keys.just_pressed(KeyCode::KeyQ);
    let mut lock_on = keys.just_pressed(KeyCode::KeyR) || mouse.just_pressed(MouseButton::Middle);
    let mut pause = keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyP);
    let mut inventory = keys.just_pressed(KeyCode::Tab) || keys.just_pressed(KeyCode::KeyC);
    let mut confirm = keys.just_pressed(KeyCode::KeyE)
        || keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space)
        || (locked && mouse.just_pressed(MouseButton::Left));
    let mut zoom = -scroll.delta.y.signum() * scroll.delta.y.abs().min(3.0);

    let touch = if c.touch_ui { take_touch().unwrap_or_default() } else { TouchFrame::default() };
    let tb = |bit: u32| touch.buttons & bit != 0;
    mv += touch.stick;
    look += touch.look * Vec2::new(0.006, 0.004);
    attack |= tb(TOUCH_ATTACK);
    roll |= tb(TOUCH_ROLL);
    jump |= tb(TOUCH_JUMP);
    interact |= tb(TOUCH_TALK);
    potion |= tb(TOUCH_POTION);
    lock_on |= tb(TOUCH_LOCK);
    inventory |= tb(TOUCH_BAG);
    pause |= tb(TOUCH_MENU);
    // 会話は画面のタップか「話す」で送る
    confirm |= tb(TOUCH_TALK) || touch.tap.is_some();

    let mut using_gamepad = c.using_gamepad;
    if mv != Vec2::ZERO || attack || look != Vec2::ZERO {
        using_gamepad = false;
    }

    for pad in &gamepads {
        let ls = pad.left_stick();
        let rs = pad.right_stick();
        let dz = |v: Vec2| {
            if v.length() < 0.18 {
                Vec2::ZERO
            } else {
                v
            }
        };
        let ls = dz(ls);
        let rs = dz(rs);
        if ls != Vec2::ZERO || rs != Vec2::ZERO {
            using_gamepad = true;
        }
        mv += ls;
        look += Vec2::new(rs.x, -rs.y) * 0.05;
        let jp = |b| pad.just_pressed(b);
        if jp(GamepadButton::West) {
            attack = true;
            using_gamepad = true;
        }
        if jp(GamepadButton::South) {
            jump = true;
            confirm = true;
            using_gamepad = true;
        }
        roll |= jp(GamepadButton::East);
        interact |= jp(GamepadButton::North);
        confirm |= jp(GamepadButton::North);
        potion |= jp(GamepadButton::RightTrigger) || jp(GamepadButton::DPadUp);
        lock_on |= jp(GamepadButton::RightThumb) || jp(GamepadButton::LeftTrigger);
        pause |= jp(GamepadButton::Start);
        inventory |= jp(GamepadButton::Select);
        if pad.pressed(GamepadButton::DPadRight) {
            zoom += 0.1;
        }
        if pad.pressed(GamepadButton::DPadLeft) {
            zoom -= 0.1;
        }
        if jp(GamepadButton::DPadDown) && choice.is_none() {
            choice = Some(9); // 「次の選択肢」を示す特殊値
        }
    }

    *c = Controls {
        movement: mv.clamp_length_max(1.0),
        look,
        zoom,
        attack,
        jump,
        roll,
        interact,
        potion,
        lock_on,
        pause,
        inventory,
        confirm,
        choice,
        using_gamepad,
        touch_ui: c.touch_ui,
        tap: touch.tap,
    };
}

/// 今の場面を body の data-mode / data-near に書き、touch.js が出すボタンを切り替えられるようにする。
fn page_mode(
    state: Res<State<GameState>>,
    block: Res<UiBlock>,
    near: Res<NearInteract>,
    controls: Res<Controls>,
    mut last: Local<(&'static str, bool)>,
) {
    if !controls.touch_ui {
        return;
    }
    let mode = match state.get() {
        GameState::Boot | GameState::Title => "title",
        GameState::Playing if block.dialog => "dialog",
        GameState::Playing if !block.any() => "play",
        _ => "menu",
    };
    let now = (mode, near.0.is_some() && mode == "play");
    if *last == now {
        return;
    }
    *last = now;
    #[cfg(target_arch = "wasm32")]
    if let Some(body) = web_sys::window().and_then(|w| w.document()).and_then(|d| d.body()) {
        let _ = body.set_attribute("data-mode", now.0);
        let _ = body.set_attribute("data-near", if now.1 { "1" } else { "0" });
    }
}

/// プレイ中はクリックでポインタロック。メニュー表示中は解除する。
fn cursor_lock(
    mut cursor: Single<&mut CursorOptions>,
    mouse: Res<ButtonInput<MouseButton>>,
    state: Res<State<GameState>>,
    block: Res<UiBlock>,
    time: Res<Time<Real>>,
    mut since_request: Local<f32>,
) {
    *since_request += time.delta_secs();
    let want_free = *state.get() != GameState::Playing || block.inventory || block.shop;
    if want_free {
        if cursor.grab_mode != CursorGrabMode::None {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
        return;
    }
    // ブラウザ側 (Esc など) で解除されたら状態を合わせ、次のクリックで掛け直せるようにする
    if cursor.grab_mode != CursorGrabMode::None && *since_request > 0.6 && !is_locked(&cursor) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
        return;
    }
    if mouse.just_pressed(MouseButton::Left) && cursor.grab_mode == CursorGrabMode::None {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
        *since_request = 0.0;
    }
}
