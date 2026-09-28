//! セーブデータ (ブラウザの localStorage)。

use bevy::prelude::*;

use crate::stats::{HuntQuest, MainQuest, PlayerStats, Progress};

const KEY: &str = "eldria_save_v1";

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, _app: &mut App) {}
}

fn bits(v: &[bool]) -> u32 {
    v.iter().enumerate().fold(0, |acc, (i, b)| if *b { acc | (1 << i) } else { acc })
}

fn unbits(n: u32, len: usize) -> Vec<bool> {
    (0..len).map(|i| n & (1 << i) != 0).collect()
}

pub fn encode(s: &PlayerStats, p: &Progress) -> String {
    let main = match p.main {
        MainQuest::NotStarted => 0,
        MainQuest::GoToRuins => 1,
        MainQuest::ReturnToElder => 2,
        MainQuest::Complete => 3,
    };
    let hunt = match p.hunt {
        HuntQuest::NotStarted => 0,
        HuntQuest::Active => 1,
        HuntQuest::Done => 2,
        HuntQuest::Rewarded => 3,
    };
    format!(
        "lv={};exp={};gold={};pot={};wb={};ab={};w={};a={};kills={};time={};main={};hunt={};wolves={};chests={};boss={}",
        s.level,
        s.exp,
        s.gold,
        s.potions,
        bits(&s.weapons),
        bits(&s.armors),
        s.weapon,
        s.armor,
        s.kills,
        s.play_time as u32,
        main,
        hunt,
        p.wolves,
        p.chests,
        p.boss_defeated as u8
    )
}

pub fn decode(text: &str) -> Option<(PlayerStats, Progress)> {
    let mut s = PlayerStats::default();
    let mut p = Progress::default();
    let mut any = false;
    for kv in text.split(';') {
        let (k, v) = kv.split_once('=')?;
        let n: u32 = v.parse().ok()?;
        any = true;
        match k {
            "lv" => s.level = n.max(1),
            "exp" => s.exp = n,
            "gold" => s.gold = n,
            "pot" => s.potions = n.min(9),
            "wb" => s.weapons = unbits(n | 1, s.weapons.len()),
            "ab" => s.armors = unbits(n | 1, s.armors.len()),
            "w" => s.weapon = n as usize,
            "a" => s.armor = n as usize,
            "kills" => s.kills = n,
            "time" => s.play_time = n as f32,
            "main" => {
                p.main = match n {
                    1 => MainQuest::GoToRuins,
                    2 => MainQuest::ReturnToElder,
                    3 => MainQuest::Complete,
                    _ => MainQuest::NotStarted,
                }
            }
            "hunt" => {
                p.hunt = match n {
                    1 => HuntQuest::Active,
                    2 => HuntQuest::Done,
                    3 => HuntQuest::Rewarded,
                    _ => HuntQuest::NotStarted,
                }
            }
            "wolves" => p.wolves = n,
            "chests" => p.chests = n,
            "boss" => p.boss_defeated = n != 0,
            _ => {}
        }
    }
    if !any {
        return None;
    }
    if s.weapon >= s.weapons.len() || !s.weapons[s.weapon] {
        s.weapon = 0;
    }
    if s.armor >= s.armors.len() || !s.armors[s.armor] {
        s.armor = 0;
    }
    s.hp = s.max_hp();
    s.stamina = s.max_stamina();
    Some((s, p))
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
pub fn write_save(s: &PlayerStats, p: &Progress) {
    if let Some(st) = storage() {
        let _ = st.set_item(KEY, &encode(s, p));
    }
}

#[cfg(target_arch = "wasm32")]
pub fn read_save() -> Option<(PlayerStats, Progress)> {
    let text = storage()?.get_item(KEY).ok()??;
    decode(&text)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn write_save(s: &PlayerStats, p: &Progress) {
    let _ = std::fs::write(std::env::temp_dir().join(KEY), encode(s, p));
}

#[cfg(not(target_arch = "wasm32"))]
pub fn read_save() -> Option<(PlayerStats, Progress)> {
    let text = std::fs::read_to_string(std::env::temp_dir().join(KEY)).ok()?;
    decode(&text)
}

pub fn has_save() -> bool {
    read_save().is_some()
}
