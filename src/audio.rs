//! 効果音と BGM。音源は tools/gen_audio.py で合成した WAV。

use bevy::{audio::Volume, prelude::*};

use crate::{
    GameState,
    util::GameRng,
    zone::{ZoneEntered, ZoneId},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SfxKind {
    Swing,
    SwingHeavy,
    Hit,
    Crit,
    EnemyDie,
    PlayerHurt,
    PlayerDie,
    Jump,
    Land,
    Roll,
    Step,
    Potion,
    Heal,
    LevelUp,
    Coin,
    Pickup,
    Chest,
    Portal,
    Talk,
    Select,
    Deny,
    Quest,
    Growl,
    Slime,
    Slam,
    Roar,
    BossGrunt,
    Magic,
}

impl SfxKind {
    fn file(self) -> (&'static str, f32) {
        use SfxKind::*;
        match self {
            Swing => ("swing", 0.5),
            SwingHeavy => ("swing_heavy", 0.6),
            Hit => ("hit", 0.6),
            Crit => ("crit", 0.7),
            EnemyDie => ("enemy_die", 0.55),
            PlayerHurt => ("hurt", 0.6),
            PlayerDie => ("player_die", 0.7),
            Jump => ("jump", 0.35),
            Land => ("land", 0.35),
            Roll => ("roll", 0.45),
            Step => ("step", 0.12),
            Potion => ("potion", 0.5),
            Heal => ("heal", 0.5),
            LevelUp => ("levelup", 0.6),
            Coin => ("coin", 0.35),
            Pickup => ("pickup", 0.5),
            Chest => ("chest", 0.6),
            Portal => ("portal", 0.6),
            Talk => ("talk", 0.35),
            Select => ("select", 0.35),
            Deny => ("deny", 0.4),
            Quest => ("quest", 0.6),
            Growl => ("growl", 0.5),
            Slime => ("slime", 0.45),
            Slam => ("slam", 0.8),
            Roar => ("roar", 0.8),
            BossGrunt => ("grunt", 0.5),
            Magic => ("magic", 0.55),
        }
    }
}

#[derive(Message, Clone, Copy)]
pub struct PlaySfx(pub SfxKind);

#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Music {
    #[default]
    None,
    Field,
    Dungeon,
    Boss,
    Victory,
}

impl Music {
    fn file(self) -> Option<&'static str> {
        match self {
            Music::None => None,
            Music::Field => Some("audio/bgm_field.wav"),
            Music::Dungeon => Some("audio/bgm_dungeon.wav"),
            Music::Boss => Some("audio/bgm_boss.wav"),
            Music::Victory => Some("audio/bgm_victory.wav"),
        }
    }
}

#[derive(Resource)]
struct SfxHandles(Vec<(SfxKind, Handle<AudioSource>)>);

#[derive(Component)]
struct BgmTrack {
    fade: f32,
    out: bool,
    vol: f32,
}

#[derive(Resource, Default)]
struct PlayingMusic(Music);

#[derive(Resource)]
pub struct AudioSettings {
    pub master: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self { master: 0.8 }
    }
}

pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySfx>()
            .init_resource::<Music>()
            .init_resource::<PlayingMusic>()
            .init_resource::<AudioSettings>()
            .add_systems(Startup, load_sfx)
            .add_systems(Update, (zone_music, play_sfx, update_music, fade_tracks));
    }
}

const ALL: [SfxKind; 28] = {
    use SfxKind::*;
    [
        Swing, SwingHeavy, Hit, Crit, EnemyDie, PlayerHurt, PlayerDie, Jump, Land, Roll, Step, Potion, Heal, LevelUp,
        Coin, Pickup, Chest, Portal, Talk, Select, Deny, Quest, Growl, Slime, Slam, Roar, BossGrunt, Magic,
    ]
};

fn load_sfx(mut commands: Commands, server: Res<AssetServer>) {
    let v = ALL
        .iter()
        .map(|k| (*k, server.load(format!("audio/{}.wav", k.file().0))))
        .collect();
    commands.insert_resource(SfxHandles(v));
}

fn play_sfx(
    mut commands: Commands,
    mut msgs: MessageReader<PlaySfx>,
    handles: Res<SfxHandles>,
    settings: Res<AudioSettings>,
    mut rng: ResMut<GameRng>,
    mut last: Local<Vec<(SfxKind, f64)>>,
    time: Res<Time<Real>>,
) {
    let now = time.elapsed_secs_f64();
    last.retain(|(_, t)| now - *t < 0.05);
    for PlaySfx(k) in msgs.read() {
        // 同じ音の同時多重再生を抑える
        if last.iter().any(|(lk, _)| lk == k) {
            continue;
        }
        last.push((*k, now));
        let Some((_, h)) = handles.0.iter().find(|(kk, _)| kk == k) else { continue };
        let (_, vol) = k.file();
        let speed = match k {
            SfxKind::Swing | SfxKind::Hit | SfxKind::Step | SfxKind::Coin | SfxKind::Talk => rng.0.range(0.9, 1.12),
            _ => rng.0.range(0.97, 1.03),
        };
        commands.spawn((
            AudioPlayer::new(h.clone()),
            PlaybackSettings::DESPAWN
                .with_volume(Volume::Linear(vol * settings.master))
                .with_speed(speed),
        ));
    }
}

fn zone_music(
    mut msgs: MessageReader<ZoneEntered>,
    mut music: ResMut<Music>,
    state: Res<State<GameState>>,
) {
    for m in msgs.read() {
        if *state.get() != GameState::Playing {
            continue;
        }
        *music = match m.0 {
            ZoneId::Field => Music::Field,
            ZoneId::Dungeon => Music::Dungeon,
        };
    }
}

fn update_music(
    mut commands: Commands,
    music: Res<Music>,
    mut playing: ResMut<PlayingMusic>,
    server: Res<AssetServer>,
    settings: Res<AudioSettings>,
    mut tracks: Query<&mut BgmTrack>,
) {
    if playing.0 == *music {
        return;
    }
    playing.0 = *music;
    for mut t in &mut tracks {
        t.out = true;
    }
    if let Some(f) = music.file() {
        let vol = if *music == Music::Victory { 0.5 } else { 0.42 } * settings.master;
        let settings = if *music == Music::Victory { PlaybackSettings::DESPAWN } else { PlaybackSettings::LOOP };
        commands.spawn((
            AudioPlayer::new(server.load(f)),
            settings.with_volume(Volume::SILENT),
            BgmTrack { fade: 0.0, out: false, vol },
        ));
    }
}

fn fade_tracks(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut q: Query<(Entity, &mut BgmTrack, Option<&mut AudioSink>)>,
) {
    let dt = time.delta_secs();
    for (e, mut t, sink) in &mut q {
        if t.out {
            t.fade -= dt / 1.2;
        } else {
            t.fade = (t.fade + dt / 1.5).min(1.0);
        }
        if let Some(mut s) = sink {
            s.set_volume(Volume::Linear(t.vol * t.fade.max(0.0)));
        }
        if t.out && t.fade <= 0.0 {
            commands.entity(e).despawn();
        }
    }
}
