//! キーボード・マウス・ゲームパッド入力を 1 つの `Controls` にまとめる。

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

use crate::{GameState, UiBlock};

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
}

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Controls>()
            .add_systems(PreUpdate, gather_input.after(bevy::input::InputSystems))
            .add_systems(Update, cursor_lock);
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

fn gather_input(
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
    };
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
