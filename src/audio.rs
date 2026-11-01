use std::{
    fs::File,
    time::{Duration, Instant},
};

use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};

use crate::beatmap::OsuMap;

const FADE_DURATION: Duration = Duration::from_millis(90);
const VOLUME_RANGE_DB: f32 = 50.0;

#[derive(Debug, Clone, Copy, PartialEq)]
enum AfterFade {
    None,
    Pause,
    Clear,
}

#[derive(Debug, Clone, Copy)]
struct Fade {
    from: f32,
    to: f32,
    start: Instant,
    after: AfterFade,
}

pub struct Audio {
    _sink: MixerDeviceSink,
    player: Player,
    volume: f32,
    gain: f32,
    fade: Option<Fade>,

    duration: Option<Duration>,
}

impl Audio {
    pub fn new() -> Option<Self> {
        let mut sink = DeviceSinkBuilder::open_default_sink().ok()?;
        sink.log_on_drop(false);
        let player = Player::connect_new(sink.mixer());

        let audio = Self {
            _sink: sink,
            player,
            volume: 0.5,
            gain: 1.0,
            fade: None,

            duration: None,
        };
        audio.apply_volume();

        Some(audio)
    }

    pub fn update(&mut self) {
        let Some(fade) = self.fade else { return };

        let t = (fade.start.elapsed().as_secs_f32() / FADE_DURATION.as_secs_f32()).min(1.0);
        self.gain = fade.from + (fade.to - fade.from) * t;

        if t >= 1.0 {
            self.fade = None;
            match fade.after {
                AfterFade::None => {}
                AfterFade::Pause => self.player.pause(),
                AfterFade::Clear => {
                    self.player.clear();
                    self.gain = 1.0;
                }
            }
        }
        self.apply_volume();
    }

    pub fn play(&mut self, map: &OsuMap) -> bool {
        if let Ok(file) = File::open(map.audio_path())
            && let Ok(src) = rodio::Decoder::try_from(file)
        {
            self.fade = None;
            self.gain = 1.0;
            self.apply_volume();
            self.duration = src.total_duration();

            self.player.clear();
            self.player.append(src);
            self.player.play();
            return true;
        }
        false
    }

    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }

    pub fn toggle_pause(&mut self) {
        if self.after_fade() == Some(AfterFade::Clear) {
            return;
        }

        if self.is_paused() {
            self.player.play();
            self.start_fade(1.0, AfterFade::None);
        } else {
            self.start_fade(0.0, AfterFade::Pause);
        }
    }

    pub fn stop(&mut self) {
        if self.player.empty() || self.player.is_paused() {
            self.player.clear();
            return;
        }

        if self.after_fade() != Some(AfterFade::Clear) {
            self.start_fade(0.0, AfterFade::Clear);
        }
    }

    pub fn position(&self) -> Duration {
        self.player.get_pos()
    }

    pub fn seek(&self, pos: Duration) {
        if self.player.empty() {
            return;
        }

        let _ = self.player.try_seek(pos);
    }

    pub fn is_paused(&self) -> bool {
        self.player.is_paused() || self.after_fade() == Some(AfterFade::Pause)
    }

    pub fn is_empty(&self) -> bool {
        self.player.empty()
    }

    pub fn is_fading(&self) -> bool {
        self.fade.is_some()
    }

    fn after_fade(&self) -> Option<AfterFade> {
        self.fade.map(|f| f.after)
    }

    fn start_fade(&mut self, to: f32, after: AfterFade) {
        self.fade = Some(Fade {
            from: self.gain,
            to,
            start: Instant::now(),
            after,
        });
    }

    pub fn add_volume(&mut self, delta: f32) {
        self.volume = (self.volume + delta).clamp(0.0, 1.0);
        self.apply_volume();
    }

    fn apply_volume(&self) {
        let amp = if self.volume < 0.01 {
            0.0
        } else {
            10f32.powf((self.volume - 1.0) * VOLUME_RANGE_DB / 20.0)
        };
        self.player.set_volume(amp * self.gain);
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }
}
