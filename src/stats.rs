//! プレイヤーのステータス・装備・進行度。

use bevy::prelude::*;

pub struct Equip {
    pub name: &'static str,
    pub desc: &'static str,
    pub atk: i32,
    pub def: i32,
    pub hp: i32,
}

pub const WEAPONS: &[Equip] = &[
    Equip { name: "旅人の剣", desc: "使い慣れた片手剣。", atk: 0, def: 0, hp: 0 },
    Equip { name: "鉄の剣", desc: "よく鍛えられた鉄の剣。", atk: 7, def: 0, hp: 0 },
    Equip { name: "騎士の剣", desc: "古の騎士団の蒼く光る剣。", atk: 16, def: 0, hp: 0 },
];

pub const ARMORS: &[Equip] = &[
    Equip { name: "布の服", desc: "ただの服。", atk: 0, def: 0, hp: 0 },
    Equip { name: "革の鎧", desc: "軽くて動きやすい。", atk: 0, def: 4, hp: 15 },
    Equip { name: "狩人の外套", desc: "森の狩人が愛用する外套。", atk: 2, def: 6, hp: 25 },
    Equip { name: "鋼の鎧", desc: "重いが頑丈な鎧。", atk: 0, def: 10, hp: 40 },
    Equip { name: "ミスリルの鎧", desc: "伝説の銀で作られた鎧。", atk: 3, def: 16, hp: 70 },
];

#[derive(Resource, Clone)]
pub struct PlayerStats {
    pub level: u32,
    pub exp: u32,
    pub hp: f32,
    pub stamina: f32,
    pub gold: u32,
    pub potions: u32,
    pub weapons: Vec<bool>,
    pub armors: Vec<bool>,
    pub weapon: usize,
    pub armor: usize,
    pub kills: u32,
    pub play_time: f32,
}

impl Default for PlayerStats {
    fn default() -> Self {
        let mut s = Self {
            level: 1,
            exp: 0,
            hp: 0.0,
            stamina: 100.0,
            gold: 30,
            potions: 2,
            weapons: vec![false; WEAPONS.len()],
            armors: vec![false; ARMORS.len()],
            weapon: 0,
            armor: 0,
            kills: 0,
            play_time: 0.0,
        };
        s.weapons[0] = true;
        s.armors[0] = true;
        s.hp = s.max_hp();
        s
    }
}

impl PlayerStats {
    pub fn max_hp(&self) -> f32 {
        (90 + self.level as i32 * 14 + ARMORS[self.armor].hp) as f32
    }
    pub fn max_stamina(&self) -> f32 {
        100.0 + self.level as f32 * 3.0
    }
    pub fn atk(&self) -> f32 {
        (8 + self.level as i32 * 3 + WEAPONS[self.weapon].atk + ARMORS[self.armor].atk) as f32
    }
    pub fn def(&self) -> f32 {
        (2 + self.level as i32 + ARMORS[self.armor].def) as f32
    }
    pub fn exp_to_next(&self) -> u32 {
        (18.0 * (self.level as f32).powf(1.55)) as u32 + 12
    }
    /// 経験値を加え、上がったレベル数を返す。
    pub fn add_exp(&mut self, amount: u32) -> u32 {
        self.exp += amount;
        let mut ups = 0;
        while self.exp >= self.exp_to_next() {
            self.exp -= self.exp_to_next();
            self.level += 1;
            ups += 1;
        }
        if ups > 0 {
            self.hp = self.max_hp();
            self.stamina = self.max_stamina();
        }
        ups
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainQuest {
    #[default]
    NotStarted,
    GoToRuins,
    ReturnToElder,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HuntQuest {
    #[default]
    NotStarted,
    Active,
    Done,
    Rewarded,
}

#[derive(Resource, Default, Clone)]
pub struct Progress {
    pub main: MainQuest,
    pub hunt: HuntQuest,
    pub wolves: u32,
    pub chests: u32,
    pub boss_defeated: bool,
}

pub const HUNT_TARGET: u32 = 5;

impl Progress {
    pub fn chest_opened(&self, id: u8) -> bool {
        self.chests & (1 << id) != 0
    }
    pub fn quest_text(&self) -> Vec<String> {
        let mut v = Vec::new();
        match self.main {
            MainQuest::NotStarted => v.push("村の長老に話しかけよう".into()),
            MainQuest::GoToRuins => v.push("北東の遺跡で黒騎士を倒せ".into()),
            MainQuest::ReturnToElder => v.push("長老に報告しよう".into()),
            MainQuest::Complete => v.push("エルディアに平和が戻った".into()),
        }
        match self.hunt {
            HuntQuest::Active => v.push(format!("狼を討伐 {}/{}", self.wolves.min(HUNT_TARGET), HUNT_TARGET)),
            HuntQuest::Done => v.push("狩人に報告しよう".into()),
            _ => {}
        }
        v
    }
}

pub struct StatsPlugin;

impl Plugin for StatsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerStats>()
            .init_resource::<Progress>()
            .add_systems(Update, tick_play_time.run_if(in_state(crate::GameState::Playing)));
    }
}

fn tick_play_time(time: Res<Time<Real>>, mut s: ResMut<PlayerStats>) {
    s.play_time += time.delta_secs();
}
