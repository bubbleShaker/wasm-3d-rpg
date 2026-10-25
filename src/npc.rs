//! NPC・会話・ショップ・宝箱・ポータルなど「調べられる」もの。

use bevy::prelude::*;

use crate::{
    GameState, UiBlock,
    assets::GameAssets,
    audio::{PlaySfx, SfxKind},
    dungeon::DUNGEON_SPAWN,
    field::RUINS_RETURN,
    gameplay_active,
    input::Controls,
    player::{PAction, Player, PlayerCtl},
    rig::{self, Look, Pose, PoseAction},
    save,
    stats::{ARMORS, HUNT_TARGET, HuntQuest, MainQuest, PlayerStats, Progress, WEAPONS},
    ui::Banner,
    util::{GameRng, damp, lerp_angle},
    vfx::{Burst, burst},
    zone::{ZoneEntity, ZoneId, ZoneRequest},
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NpcId {
    Elder,
    Merchant,
    Hunter,
    Child,
}

impl NpcId {
    pub fn name(self) -> &'static str {
        match self {
            NpcId::Elder => "長老オルド",
            NpcId::Merchant => "商人ミラ",
            NpcId::Hunter => "狩人ガレス",
            NpcId::Child => "村の子ども ピピ",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InteractKind {
    Npc(NpcId),
    Chest(u8),
    Portal,
    ReturnPortal,
    SaveCrystal,
}

#[derive(Component)]
pub struct Interactable {
    pub kind: InteractKind,
    pub radius: f32,
}

#[allow(dead_code)]
#[derive(Component)]
pub struct Npc {
    pub id: NpcId,
    pub facing: f32,
    pub base_facing: f32,
}

#[allow(dead_code)]
#[derive(Component)]
pub struct Chest {
    pub id: u8,
    pub opened: bool,
    pub lid: Entity,
    pub anim: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DAction {
    AcceptMain,
    AcceptHunt,
    RewardHunt,
    Victory,
    BuyPotion,
    BuySword,
    BuyArmor,
    Close,
}

#[derive(Resource, Default)]
pub struct Dialog {
    pub active: bool,
    pub speaker: String,
    pub pages: Vec<String>,
    pub page: usize,
    pub shown: f32,
    pub choices: Vec<(String, DAction)>,
    pub selected: usize,
    pub on_end: Option<DAction>,
    pub npc: Option<Entity>,
    /// 閉じた直後に同じキーで話しかけ直さないための待ち時間
    pub cooldown: f32,
}

impl Dialog {
    pub fn page_text(&self) -> &str {
        self.pages.get(self.page).map(|s| s.as_str()).unwrap_or("")
    }
    pub fn page_done(&self) -> bool {
        self.shown as usize >= self.page_text().chars().count()
    }
    pub fn at_last(&self) -> bool {
        self.page + 1 >= self.pages.len()
    }
}

/// いま近くにある調べられるもの (UI の案内表示用)。
#[derive(Resource, Default)]
pub struct NearInteract(pub Option<String>);

/// 画面暗転つきのゾーン移動。
#[derive(Resource, Default)]
pub struct Transition {
    pub pending: Option<ZoneRequest>,
    pub t: f32,
    pub fired: bool,
}

pub struct NpcPlugin;

impl Plugin for NpcPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dialog>()
            .init_resource::<NearInteract>()
            .init_resource::<Transition>()
            .add_systems(
                Update,
                (
                    interact.run_if(gameplay_active),
                    dialog_input.run_if(in_state(GameState::Playing)),
                    npc_idle,
                    chest_anim,
                    transition,
                ),
            );
    }
}

pub fn spawn_npc(commands: &mut Commands, ga: &GameAssets, id: NpcId, pos: Vec3, facing: f32) {
    let e = commands
        .spawn((
            Npc { id, facing, base_facing: facing },
            Interactable { kind: InteractKind::Npc(id), radius: 2.6 },
            Pose::default(),
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(facing)),
            Visibility::default(),
            ZoneEntity,
        ))
        .id();
    let mut look = Look::basic(ga);
    let robe = |c: Color| StandardMaterial { base_color: c, perceptual_roughness: 0.85, ..default() };
    // NPC 用の色は初回だけ作ればよいが、数が少ないのでそのまま作る
    let _ = robe;
    match id {
        NpcId::Elder => {
            look.torso = ga.knight_cape.clone();
            look.arms = ga.knight_cape.clone();
            look.apron = Some(ga.knight_cape.clone());
            look.hair = Some(ga.bone.clone());
            look.beard = Some(ga.bone.clone());
            look.staff = Some(ga.leather.clone());
            look.scale = 0.95;
        }
        NpcId::Merchant => {
            look.torso = ga.cape.clone();
            look.arms = ga.skin.clone();
            look.apron = Some(ga.bone.clone());
            look.hat = Some(ga.leather.clone());
        }
        NpcId::Hunter => {
            look.torso = ga.golem_moss.clone();
            look.arms = ga.golem_moss.clone();
            look.apron = Some(ga.leather.clone());
            look.hair = Some(ga.dark.clone());
            look.cape = Some(ga.wolf_fur.clone());
            look.bulk = 1.15;
        }
        NpcId::Child => {
            look.scale = 0.62;
            look.torso = ga.slime_core.clone();
            look.arms = ga.skin.clone();
            look.hair = Some(ga.gold.clone());
        }
    }
    rig::build_humanoid(commands, ga, e, &look);
}

pub fn spawn_chest(commands: &mut Commands, ga: &GameAssets, id: u8, pos: Vec3, facing: f32, opened: bool) {
    let lid = commands
        .spawn((
            Transform::from_xyz(0.0, 0.62, 0.32).with_rotation(Quat::from_rotation_x(if opened { -1.9 } else { 0.0 })),
            Visibility::default(),
        ))
        .id();
    let lid_parts = [
        commands.spawn((Mesh3d(ga.cube.clone()), MeshMaterial3d(ga.leather.clone()), Transform::from_xyz(0.0, 0.1, -0.32).with_scale(Vec3::new(1.0, 0.22, 0.66)))).id(),
        commands.spawn((Mesh3d(ga.cube.clone()), MeshMaterial3d(ga.gold.clone()), Transform::from_xyz(0.0, 0.1, -0.32).with_scale(Vec3::new(1.04, 0.1, 0.68)))).id(),
        commands.spawn((Mesh3d(ga.cube.clone()), MeshMaterial3d(ga.gold.clone()), Transform::from_xyz(0.0, 0.05, 0.02).with_scale(Vec3::new(0.14, 0.18, 0.06)))).id(),
    ];
    commands.entity(lid).add_children(&lid_parts);
    let e = commands
        .spawn((
            Chest { id, opened, lid, anim: if opened { 1.0 } else { 0.0 } },
            Interactable { kind: InteractKind::Chest(id), radius: 2.2 },
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(facing)),
            Visibility::default(),
            ZoneEntity,
        ))
        .id();
    let body = [
        commands.spawn((Mesh3d(ga.cube.clone()), MeshMaterial3d(ga.leather.clone()), Transform::from_xyz(0.0, 0.32, 0.0).with_scale(Vec3::new(1.0, 0.6, 0.64)))).id(),
        commands.spawn((Mesh3d(ga.cube.clone()), MeshMaterial3d(ga.gold.clone()), Transform::from_xyz(0.0, 0.32, 0.0).with_scale(Vec3::new(0.12, 0.62, 0.66)))).id(),
        commands.spawn((Mesh3d(ga.cube.clone()), MeshMaterial3d(ga.gold.clone()), Transform::from_xyz(0.0, 0.05, 0.0).with_scale(Vec3::new(1.04, 0.1, 0.68)))).id(),
    ];
    commands.entity(e).add_children(&body);
    commands.entity(e).add_child(lid);
}

fn chest_anim(time: Res<Time>, mut chests: Query<&mut Chest>, mut tf: Query<&mut Transform>) {
    for mut c in &mut chests {
        if c.opened && c.anim < 1.0 {
            c.anim = (c.anim + time.delta_secs() * 2.5).min(1.0);
            if let Ok(mut t) = tf.get_mut(c.lid) {
                let u = c.anim;
                let e = 1.0 - (1.0 - u) * (1.0 - u);
                t.rotation = Quat::from_rotation_x(-1.9 * e);
            }
        }
    }
}

fn npc_idle(
    time: Res<Time>,
    dialog: Res<Dialog>,
    player: Query<&Transform, (With<Player>, Without<Npc>)>,
    mut q: Query<(Entity, &mut Transform, &mut Npc, &mut Pose)>,
) {
    let dt = time.delta_secs();
    let ppos = player.single().map(|t| t.translation).unwrap_or(Vec3::ZERO);
    for (e, mut tf, mut npc, mut pose) in &mut q {
        pose.clock += dt;
        let d = ppos - tf.translation;
        let want = if d.length() < 6.0 { (-d.x).atan2(-d.z) } else { npc.base_facing };
        npc.facing = lerp_angle(npc.facing, want, damp(4.0, dt));
        tf.rotation = Quat::from_rotation_y(npc.facing);
        let talking = dialog.active && dialog.npc == Some(e);
        pose.action = if talking { PoseAction::Talk } else { PoseAction::None };
    }
}

fn open_dialog(dialog: &mut Dialog, speaker: &str, pages: Vec<String>, choices: Vec<(String, DAction)>, on_end: Option<DAction>, npc: Option<Entity>) {
    *dialog = Dialog {
        active: true,
        speaker: speaker.into(),
        pages,
        page: 0,
        shown: 0.0,
        choices,
        selected: 0,
        on_end,
        npc,
        cooldown: 0.0,
    };
}

fn shop_choices(stats: &PlayerStats) -> Vec<(String, DAction)> {
    let mut v = vec![("回復薬を買う (25G)".to_string(), DAction::BuyPotion)];
    if !stats.weapons[1] {
        v.push(("鉄の剣を買う (150G)".into(), DAction::BuySword));
    }
    if !stats.armors[3] {
        v.push(("鋼の鎧を買う (260G)".into(), DAction::BuyArmor));
    }
    v.push(("やめる".into(), DAction::Close));
    v
}

fn npc_dialog(id: NpcId, progress: &Progress, stats: &PlayerStats, child_idx: &mut usize) -> (Vec<String>, Vec<(String, DAction)>, Option<DAction>) {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    match id {
        NpcId::Elder => match progress.main {
            MainQuest::NotStarted => (
                s(&[
                    "おお、旅の剣士殿。よくぞこのエルディア村へ参られた。",
                    "実は北東の古代遺跡で、封印されていた黒騎士ヴァルガが目覚めてしもうたのじゃ…",
                    "森の魔物たちが凶暴になったのも、奴の放つ瘴気のせいに違いない。",
                    "どうか遺跡の奥で黒騎士を討ってくだされ。少ないが、これは餞別じゃ。",
                ]),
                vec![],
                Some(DAction::AcceptMain),
            ),
            MainQuest::GoToRuins => (
                s(&[
                    "遺跡は村から北東へ、道なりに進んだ先の台地にある。",
                    "紫に輝く門をくぐれば遺跡の中へ入れるはずじゃ。",
                    "広場の青いクリスタルに触れれば、傷を癒やし旅の記録を残せるぞ。",
                ]),
                vec![],
                None,
            ),
            MainQuest::ReturnToElder => (
                s(&[
                    "なんと…! 黒騎士ヴァルガを討ち果たしたと申すか!",
                    "たしかに瘴気が晴れてゆくのを感じる。そなたこそ真の英雄じゃ。",
                    "エルディアの民を代表して礼を言う。本当に、ありがとう。",
                ]),
                vec![],
                Some(DAction::Victory),
            ),
            MainQuest::Complete => (
                s(&["英雄殿、ゆっくり休んでいかれよ。", "魔物の残党はまだおる。腕を磨くのも良かろう。"]),
                vec![],
                None,
            ),
        },
        NpcId::Merchant => (
            vec![format!("いらっしゃい! 旅の支度ならここで揃うよ。(所持金 {}G)", stats.gold)],
            shop_choices(stats),
            None,
        ),
        NpcId::Hunter => match progress.hunt {
            HuntQuest::NotStarted => (
                s(&["最近、森の狼どもが村の近くまで出てきやがる。", "5 頭ほど狩ってくれねえか? 礼ははずむぜ。"]),
                vec![("引き受ける".into(), DAction::AcceptHunt), ("今は無理だ".into(), DAction::Close)],
                None,
            ),
            HuntQuest::Active => (
                vec![format!("狼は東の森や北の森に多い。気をつけな。({}/{})", progress.wolves, HUNT_TARGET)],
                vec![],
                None,
            ),
            HuntQuest::Done => (
                s(&["やるじゃねえか! 約束の品だ。", "俺の自慢の『狩人の外套』、持っていきな。"]),
                vec![],
                Some(DAction::RewardHunt),
            ),
            HuntQuest::Rewarded => (s(&["あんたの腕なら、黒騎士だって倒せるかもな。"]), vec![], None),
        },
        NpcId::Child => {
            let tips = [
                "ねえねえ、回避で転がると敵の攻撃をよけられるんだって!",
                "ロックオンで敵に狙いを定められるんだよ。囲まれたときに便利!",
                "回復薬はピンチのときにすぐ飲んでね!",
                "攻撃を続けて押すと 3 段斬り! 最後の一撃はすっごく強いんだ!",
                "赤く光る地面は危ないよ! すぐに離れて!",
                "持ち物を開くと装備を着替えられるよ。いい装備を拾ったら着替えてね。",
            ];
            let t = tips[*child_idx % tips.len()];
            *child_idx += 1;
            (vec![t.to_string()], vec![], None)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn interact(
    controls: Res<Controls>,
    mut commands: Commands,
    ga: Res<GameAssets>,
    mut rng: ResMut<GameRng>,
    player: Query<(&Transform, &PlayerCtl), With<Player>>,
    things: Query<(Entity, &GlobalTransform, &Interactable)>,
    mut chests: Query<&mut Chest>,
    mut near: ResMut<NearInteract>,
    mut dialog: ResMut<Dialog>,
    mut block: ResMut<UiBlock>,
    mut stats: ResMut<PlayerStats>,
    mut progress: ResMut<Progress>,
    mut sfx: MessageWriter<PlaySfx>,
    mut banner: MessageWriter<Banner>,
    mut trans: ResMut<Transition>,
    mut child_idx: Local<usize>,
) {
    let Ok((pt, ctl)) = player.single() else { return };
    let mut best: Option<(Entity, InteractKind, f32)> = None;
    for (e, gt, it) in &things {
        let d = gt.translation().xz().distance(pt.translation.xz());
        if d < it.radius && best.is_none_or(|b| d < b.2) {
            if let InteractKind::Chest(_) = it.kind
                && chests.get(e).map(|c| c.opened).unwrap_or(false)
            {
                continue;
            }
            best = Some((e, it.kind, d));
        }
    }
    near.0 = best.map(|(_, k, _)| match k {
        InteractKind::Npc(id) => format!("話す: {}", id.name()),
        InteractKind::Chest(_) => "宝箱を開ける".into(),
        InteractKind::Portal => "遺跡に入る".into(),
        InteractKind::ReturnPortal => "地上へ戻る".into(),
        InteractKind::SaveCrystal => "クリスタルに触れる (回復・記録)".into(),
    });
    if !controls.interact || ctl.action != PAction::Idle || trans.pending.is_some() || dialog.cooldown > 0.0 {
        return;
    }
    let Some((e, kind, _)) = best else { return };
    match kind {
        InteractKind::Npc(id) => {
            let (pages, choices, on_end) = npc_dialog(id, &progress, &stats, &mut child_idx);
            open_dialog(&mut dialog, id.name(), pages, choices, on_end, Some(e));
            block.dialog = true;
            sfx.write(PlaySfx(SfxKind::Talk));
        }
        InteractKind::Chest(id) => {
            if let Ok(mut c) = chests.get_mut(e) {
                c.opened = true;
            }
            progress.chests |= 1 << id;
            sfx.write(PlaySfx(SfxKind::Chest));
            if let Ok((_, gt, _)) = things.get(e) {
                burst(&mut commands, &ga, &mut rng.0, Burst::Chest, gt.translation() + Vec3::Y * 0.8);
            }
            let msg = chest_reward(id, &mut stats);
            banner.write(Banner::small(&msg));
        }
        InteractKind::Portal => {
            trans.pending = Some(ZoneRequest { zone: ZoneId::Dungeon, spawn: DUNGEON_SPAWN, facing: 0.0 });
            trans.t = 0.0;
            trans.fired = false;
            sfx.write(PlaySfx(SfxKind::Portal));
        }
        InteractKind::ReturnPortal => {
            trans.pending = Some(ZoneRequest { zone: ZoneId::Field, spawn: RUINS_RETURN, facing: 2.4 });
            trans.t = 0.0;
            trans.fired = false;
            sfx.write(PlaySfx(SfxKind::Portal));
        }
        InteractKind::SaveCrystal => {
            stats.hp = stats.max_hp();
            stats.stamina = stats.max_stamina();
            save::write_save(&stats, &progress);
            sfx.write(PlaySfx(SfxKind::Heal));
            burst(&mut commands, &ga, &mut rng.0, Burst::Heal, pt.translation + Vec3::Y);
            banner.write(Banner::small("HP が回復した。旅の記録を残した。"));
        }
    }
}

fn chest_reward(id: u8, s: &mut PlayerStats) -> String {
    match id {
        0 => {
            s.gold += 60;
            s.potions = (s.potions + 1).min(9);
            "60G と 回復薬 を手に入れた!".into()
        }
        1 => {
            s.armors[1] = true;
            format!("{} を手に入れた! (持ち物から装備)", ARMORS[1].name)
        }
        2 => {
            s.gold += 100;
            s.potions = (s.potions + 2).min(9);
            "100G と 回復薬 x2 を手に入れた!".into()
        }
        3 => {
            s.gold += 150;
            "150G を手に入れた!".into()
        }
        10 => {
            s.weapons[2] = true;
            format!("{} を手に入れた! (持ち物から装備)", WEAPONS[2].name)
        }
        11 => {
            s.armors[4] = true;
            s.potions = (s.potions + 2).min(9);
            format!("{} と 回復薬 x2 を手に入れた!", ARMORS[4].name)
        }
        _ => "何も入っていなかった…".into(),
    }
}

#[allow(clippy::too_many_arguments)]
fn dialog_input(
    time: Res<Time<Real>>,
    controls: Res<Controls>,
    keys: Res<ButtonInput<KeyCode>>,
    mut dialog: ResMut<Dialog>,
    mut block: ResMut<UiBlock>,
    mut stats: ResMut<PlayerStats>,
    mut progress: ResMut<Progress>,
    mut sfx: MessageWriter<PlaySfx>,
    mut banner: MessageWriter<Banner>,
    mut next: ResMut<NextState<GameState>>,
    mut opened_at: Local<f32>,
) {
    if !dialog.active {
        *opened_at = 0.0;
        dialog.cooldown -= time.delta_secs();
        return;
    }
    *opened_at += time.delta_secs();
    let len = dialog.page_text().chars().count() as f32;
    dialog.shown = (dialog.shown + time.delta_secs() * 38.0).min(len);
    // 開いた直後の入力は無視 (話しかけたキーで即送りにならないように)
    if *opened_at < 0.15 {
        return;
    }
    let showing_choices = dialog.at_last() && dialog.page_done() && !dialog.choices.is_empty();
    let mut pick: Option<usize> = None;
    if showing_choices {
        let n = dialog.choices.len();
        if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) || controls.choice == Some(9) {
            dialog.selected = (dialog.selected + 1) % n;
            sfx.write(PlaySfx(SfxKind::Select));
        }
        if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) {
            dialog.selected = (dialog.selected + n - 1) % n;
            sfx.write(PlaySfx(SfxKind::Select));
        }
        if let Some(c) = controls.choice
            && c < n
        {
            pick = Some(c);
        }
        if controls.confirm {
            pick = Some(dialog.selected);
        }
    } else if controls.confirm {
        if !dialog.page_done() {
            dialog.shown = len;
        } else if !dialog.at_last() {
            dialog.page += 1;
            dialog.shown = 0.0;
            sfx.write(PlaySfx(SfxKind::Talk));
        } else {
            let end = dialog.on_end;
            dialog.active = false;
            dialog.cooldown = 0.3;
            block.dialog = false;
            if let Some(a) = end {
                run_action(a, &mut dialog, &mut block, &mut stats, &mut progress, &mut sfx, &mut banner, &mut next);
            }
        }
    }
    if let Some(i) = pick {
        let a = dialog.choices[i].1;
        dialog.active = false;
        dialog.cooldown = 0.3;
        block.dialog = false;
        run_action(a, &mut dialog, &mut block, &mut stats, &mut progress, &mut sfx, &mut banner, &mut next);
    }
}

#[allow(clippy::too_many_arguments)]
fn run_action(
    a: DAction,
    dialog: &mut Dialog,
    block: &mut UiBlock,
    stats: &mut PlayerStats,
    progress: &mut Progress,
    sfx: &mut MessageWriter<PlaySfx>,
    banner: &mut MessageWriter<Banner>,
    next: &mut NextState<GameState>,
) {
    let npc = dialog.npc;
    let reopen = |dialog: &mut Dialog, block: &mut UiBlock, msg: String, stats: &PlayerStats| {
        let choices = shop_choices(stats);
        open_dialog(dialog, NpcId::Merchant.name(), vec![msg], choices, None, npc);
        dialog.shown = 999.0;
        block.dialog = true;
    };
    match a {
        DAction::Close => {}
        DAction::AcceptMain => {
            progress.main = MainQuest::GoToRuins;
            stats.potions = (stats.potions + 3).min(9);
            stats.gold += 50;
            sfx.write(PlaySfx(SfxKind::Quest));
            banner.write(Banner::big("メインクエスト開始", "北東の遺跡で黒騎士を倒せ  (回復薬 x3 と 50G を受け取った)"));
        }
        DAction::AcceptHunt => {
            progress.hunt = HuntQuest::Active;
            sfx.write(PlaySfx(SfxKind::Quest));
            banner.write(Banner::small("サブクエスト: 森の狼を 5 頭討伐せよ"));
        }
        DAction::RewardHunt => {
            progress.hunt = HuntQuest::Rewarded;
            stats.armors[2] = true;
            stats.gold += 120;
            sfx.write(PlaySfx(SfxKind::Quest));
            banner.write(Banner::small("狩人の外套 と 120G を手に入れた!"));
        }
        DAction::Victory => {
            progress.main = MainQuest::Complete;
            next.set(GameState::Victory);
        }
        DAction::BuyPotion => {
            if stats.gold >= 25 && stats.potions < 9 {
                stats.gold -= 25;
                stats.potions += 1;
                sfx.write(PlaySfx(SfxKind::Coin));
                reopen(dialog, block, format!("毎度あり! 回復薬は今 {} 個だね。(所持金 {}G)", stats.potions, stats.gold), stats);
            } else {
                sfx.write(PlaySfx(SfxKind::Deny));
                let m = if stats.potions >= 9 { "それ以上は持てないよ。".to_string() } else { "お金が足りないみたいだね…".to_string() };
                reopen(dialog, block, m, stats);
            }
        }
        DAction::BuySword | DAction::BuyArmor => {
            let (price, name) = if a == DAction::BuySword { (150, WEAPONS[1].name) } else { (260, ARMORS[3].name) };
            if stats.gold >= price {
                stats.gold -= price;
                if a == DAction::BuySword {
                    stats.weapons[1] = true;
                } else {
                    stats.armors[3] = true;
                }
                sfx.write(PlaySfx(SfxKind::Coin));
                reopen(dialog, block, format!("{name} だね、いい買い物だよ! 持ち物から装備してね。"), stats);
            } else {
                sfx.write(PlaySfx(SfxKind::Deny));
                reopen(dialog, block, "お金が足りないみたいだね…".into(), stats);
            }
        }
    }
}

fn transition(
    time: Res<Time<Real>>,
    mut trans: ResMut<Transition>,
    mut req: MessageWriter<ZoneRequest>,
    mut player: Query<&mut PlayerCtl, With<Player>>,
) {
    if trans.pending.is_none() {
        return;
    }
    trans.t += time.delta_secs();
    if trans.t > 0.45 && !trans.fired {
        trans.fired = true;
        if let Some(r) = trans.pending {
            req.write(r);
        }
        if let Ok(mut c) = player.single_mut() {
            c.facing = trans.pending.map(|r| r.facing).unwrap_or(0.0);
            c.lock = None;
            c.vel = Vec3::ZERO;
        }
    }
    if trans.t > 1.1 {
        trans.pending = None;
        trans.t = 0.0;
    }
}

/// 暗転の不透明度 (UI 用)。
pub fn fade_alpha(t: &Transition) -> f32 {
    if t.pending.is_none() {
        return 0.0;
    }
    if t.t < 0.45 { t.t / 0.45 } else { (1.0 - (t.t - 0.55) / 0.55).clamp(0.0, 1.0) }
}
