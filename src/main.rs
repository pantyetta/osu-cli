use std::{
    env::{self},
    path::PathBuf,
    time::Duration,
};

use ratatui::{
    DefaultTerminal,
    crossterm::event::{
        self,
        Event::{self},
        KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
    },
    widgets::ListState,
};
use ratatui_image::picker::Picker;

use crate::{library::load, thumbnail::Thumbnail};
use audio::Audio;
use beatmap::OsuMap;
use playlist::{Playlist, Step};

mod audio;
mod beatmap;
mod config;
mod library;
mod playlist;
mod thumbnail;
mod ui;

const RESTART_THRESHOLD: Duration = Duration::from_secs(3);
const SEEK_STEP: Duration = Duration::from_secs(5);

#[derive(Debug)]
enum Mode {
    Normal,
    Search,
    Help,
    ConfirmQuit,
}

#[derive(Debug, Clone, Copy)]
enum Tab {
    Maps,
    Queue,
    History,
}

impl Tab {
    const ALL: [Tab; 3] = [Tab::Maps, Tab::Queue, Tab::History];

    fn next(self) -> Self {
        Self::ALL[(self as usize + 1) % Self::ALL.len()]
    }

    fn prev(self) -> Self {
        Self::ALL[(self as usize + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    fn title(self) -> &'static str {
        match self {
            Tab::Maps => "Maps",
            Tab::Queue => "Queue",
            Tab::History => "History",
        }
    }
}

struct App {
    songs: Vec<OsuMap>,
    audio: Option<Audio>,

    mode: Mode,
    tab: Tab,
    query: String,
    filtered: Vec<usize>,
    map_state: ListState,
    queue_state: ListState,
    history_state: ListState,
    page_height: u16,

    playlist: Playlist,

    thumbnail: Thumbnail,
}

impl App {
    fn new(songs: Vec<OsuMap>, picker: Picker) -> Self {
        Self {
            filtered: (0..songs.len()).collect(),
            songs,
            audio: Audio::new(),
            mode: Mode::Normal,
            tab: Tab::Maps,
            query: String::new(),
            map_state: ListState::default().with_selected(Some(0)),
            queue_state: ListState::default().with_selected(Some(0)),
            history_state: ListState::default().with_selected(Some(0)),
            page_height: 0,

            playlist: Playlist::new(),

            thumbnail: Thumbnail::new(picker),
        }
    }

    fn edit_query(&mut self, edit: impl FnOnce(&mut String)) {
        edit(&mut self.query);

        let q = self.query.to_lowercase();
        self.filtered = self
            .songs
            .iter()
            .enumerate()
            .filter(|(_, m)| m.matches(&q))
            .map(|(i, _)| i)
            .collect();
        self.map_state.select(Some(0));
    }

    pub fn playing(&self) -> Option<usize> {
        if self.audio.as_ref()?.is_empty() {
            return None;
        }

        self.playlist.current()
    }

    pub fn tick(&mut self) {
        let Some(audio) = &mut self.audio else { return };
        audio.update();
        if !audio.is_empty() {
            return;
        };

        let songs = &self.songs;
        self.playlist.advance(|song| audio.play(&songs[song]));
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match self.mode {
            Mode::Normal => self.handle_normal(key),
            Mode::Search => self.handle_search(key),
            Mode::Help => self.handle_help(key),
            Mode::ConfirmQuit => return self.handle_confirm_quit(key),
        }
        false
    }

    fn handle_normal(&mut self, key: KeyEvent) {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.query.is_empty() {
                    self.mode = Mode::ConfirmQuit;
                    return;
                }
                self.edit_query(String::clear);
            }

            KeyCode::Up if shift => self.with_audio(|a| a.add_volume(0.05)),
            KeyCode::Down if shift => self.with_audio(|a| a.add_volume(-0.05)),
            KeyCode::Left if shift => self.unless_fading(Self::play_prev),
            KeyCode::Right if shift => self.unless_fading(Self::play_next),

            KeyCode::Left => self.unless_fading(Self::seek_backward),
            KeyCode::Right => self.unless_fading(Self::seek_forward),

            KeyCode::BackTab => self.tab = self.tab.prev(),
            KeyCode::Tab => self.tab = self.tab.next(),

            KeyCode::Char('r') => self.playlist.toggle_repeat(),
            KeyCode::Char('s') => self.unless_fading(|app| app.playlist.toggle_shuffle()),
            KeyCode::Char('k') | KeyCode::Up => self.active_state().select_previous(),
            KeyCode::Char('j') | KeyCode::Down => self.active_state().select_next(),
            KeyCode::Home => self.active_state().select_first(),
            KeyCode::End => self.active_state().select_last(),
            KeyCode::PageUp => {
                let h = self.page_height;
                self.active_state().scroll_up_by(h);
            }
            KeyCode::PageDown => {
                let h = self.page_height;
                self.active_state().scroll_down_by(h);
            }

            KeyCode::Char('/') => {
                self.tab = Tab::Maps;
                self.mode = Mode::Search;
            }
            KeyCode::Char('?') => self.mode = Mode::Help,

            KeyCode::Enter => match self.tab {
                Tab::Maps => self.unless_fading(Self::play_from_maps),
                Tab::Queue => self.unless_fading(Self::jump_in_queue),
                _ => {}
            },
            KeyCode::Char(' ') => self.with_audio(|a| a.toggle_pause()),
            _ => {}
        }
    }

    fn handle_search(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.mode = Mode::Normal,
            KeyCode::Esc => {
                self.edit_query(String::clear);
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => self.edit_query(|q| {
                q.pop();
            }),
            KeyCode::Char(c) => self.edit_query(|q| q.push(c)),
            _ => {}
        }
    }

    fn handle_help(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
            self.mode = Mode::Normal;
        }
    }

    fn handle_confirm_quit(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter => return true,
            KeyCode::Char('n') | KeyCode::Char('q') | KeyCode::Esc => self.mode = Mode::Normal,
            _ => {}
        }
        false
    }

    fn active_state(&mut self) -> &mut ListState {
        match self.tab {
            Tab::Maps => &mut self.map_state,
            Tab::Queue => &mut self.queue_state,
            Tab::History => &mut self.history_state,
        }
    }

    fn with_audio(&mut self, f: impl FnOnce(&mut Audio)) {
        if let Some(audio) = &mut self.audio {
            f(audio);
        }
    }

    fn unless_fading(&mut self, action: impl FnOnce(&mut Self)) {
        if !self.is_fading() {
            action(self);
        }
    }

    fn play_from_maps(&mut self) {
        let Some(sel) = self.map_state.selected() else {
            return;
        };

        if sel >= self.filtered.len() {
            return;
        }
        let mut filtered = self.filtered.clone();
        filtered.rotate_left(sel);
        self.playlist.start(filtered);
        self.with_audio(|a| a.stop());
    }

    fn jump_in_queue(&mut self) {
        let Some(sel) = self.queue_state.selected() else {
            return;
        };

        if self.playlist.skip_upcoming(sel) {
            self.queue_state.select(Some(0));
            self.play_next();
        }
    }

    fn play_next(&mut self) {
        self.playlist.set_step(Step::Next);
        self.with_audio(|a| a.stop());
    }

    fn play_prev(&mut self) {
        let Some(audio) = &mut self.audio else { return };
        if audio.is_empty() {
            self.playlist.set_step(Step::Current);
            return;
        }

        if !self.playlist.has_prev() || audio.position() > RESTART_THRESHOLD {
            audio.seek(Duration::ZERO);
            return;
        }

        self.playlist.set_step(Step::Prev);
        audio.stop();
    }

    pub fn is_fading(&self) -> bool {
        self.audio.as_ref().is_some_and(Audio::is_fading)
    }

    fn volume(&self) -> String {
        self.audio.as_ref().map_or("---".to_string(), |a| {
            format!("{}", (a.volume() * 100.0).round() as u8)
        })
    }

    fn update_thumbnail(&mut self) {
        self.thumbnail.poll();

        if let Some(i) = self.playing() {
            let path = self.songs[i].bg_path();
            self.thumbnail.request(i, path);
        }
    }

    fn seek_backward(&mut self) {
        self.with_audio(|a| a.seek(a.position().saturating_sub(SEEK_STEP)));
    }

    fn seek_forward(&mut self) {
        let Some(audio) = &self.audio else { return };
        let target = audio.position() + SEEK_STEP;
        if audio.duration().is_some_and(|d| target > d) {
            self.play_next();
            return;
        }

        audio.seek(target);
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let rescan = args.iter().any(|arg| arg == "--rescan");

    let root = match args.iter().find(|arg| !arg.starts_with("--")) {
        Some(arg) => PathBuf::from(arg),
        None => config::load_or_setup()?,
    };
    let cache_path = dirs::cache_dir()
        .unwrap_or_else(env::temp_dir)
        .join("osu-cli")
        .join("library.json");

    let songs = load(&root, &cache_path, rescan)?;
    let terminal = ratatui::init();
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

    let mut app = App::new(songs, picker);

    let result = run(terminal, &mut app);
    ratatui::restore();
    result
}

fn run(mut terminal: DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    loop {
        app.tick();
        app.update_thumbnail();
        terminal.draw(|frame| ui::draw(frame, app))?;

        let timeout = if app.is_fading() {
            Duration::from_millis(15)
        } else {
            Duration::from_millis(150)
        };

        if !event::poll(timeout)? {
            continue;
        }

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && app.handle_key(key)
        {
            return Ok(());
        }
    }
}
