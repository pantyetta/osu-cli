use std::{
    collections::VecDeque,
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

mod audio;
mod beatmap;
mod library;
mod thumbnail;
mod ui;

const HISTORY_LIMIT: usize = 1_000;
const QUEUE_LIMIT: usize = 1_000;
const RESTART_THRESHOLD: Duration = Duration::from_secs(3);
const SEEK_STEP: Duration = Duration::from_secs(5);

#[derive(Debug)]
enum Mode {
    Normal,
    Search,
    Help,
}

#[derive(Debug, Clone, Copy)]
enum Tab {
    Maps,
    Queue,
    History,
}

#[derive(Debug, Clone, Copy)]
enum Step {
    Next,
    Prev,
    Current,
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
    map_state: ListState,
    queue_state: ListState,
    history_state: ListState,
    page_height: u16,

    original_queue: Vec<usize>,
    queue: Vec<usize>,
    history: VecDeque<usize>,
    pos: Option<usize>,
    step: Step,
    shuffle: bool,
    repeat: bool,

    thumbnail: Thumbnail,
}

impl App {
    fn new(songs: Vec<OsuMap>, picker: Picker) -> Self {
        Self {
            songs,
            audio: Audio::new(),
            mode: Mode::Normal,
            tab: Tab::Maps,
            query: String::new(),
            map_state: ListState::default().with_selected(Some(0)),
            queue_state: ListState::default().with_selected(Some(0)),
            history_state: ListState::default().with_selected(Some(0)),
            page_height: 0,

            original_queue: Vec::new(),
            queue: Vec::new(),
            history: VecDeque::with_capacity(HISTORY_LIMIT),
            pos: None,
            step: Step::Next,
            shuffle: false,
            repeat: false,

            thumbnail: Thumbnail::new(picker),
        }
    }

    pub fn filtered(&self) -> Vec<usize> {
        let q = self.query.to_lowercase();
        self.songs
            .iter()
            .enumerate()
            .filter(|(_, m)| m.matches(&q))
            .map(|(i, _)| i)
            .collect()
    }

    pub fn playing(&self) -> Option<usize> {
        if self.audio.as_ref()?.is_empty() {
            return None;
        }

        self.pos.and_then(|p| self.queue.get(p)).copied()
    }

    pub fn tick(&mut self) {
        let Some(audio) = &mut self.audio else { return };
        audio.update();
        if !audio.is_empty() {
            return;
        };

        let mut target = match self.step {
            Step::Next => self.next_cursor(),
            Step::Prev => self.pos.and_then(|p| p.checked_sub(1)),
            Step::Current => self.pos,
        };

        self.step = Step::Next;

        for _ in 0..=self.original_queue.len() {
            let Some(pos) = target else { break };
            if self.play_at(pos) {
                break;
            }
            self.pos = Some(pos);
            target = self.next_cursor();
        }
    }

    fn next_cursor(&mut self) -> Option<usize> {
        let mut next = self.upcoming_start();
        if next < self.queue.len() {
            return Some(next);
        }

        if !self.repeat || self.original_queue.is_empty() {
            return None;
        }

        let mut lap = self.original_queue.clone();

        if self.shuffle {
            fastrand::shuffle(&mut lap);

            if lap.len() > 1 && lap.first() == self.queue.last() {
                let i = fastrand::usize(1..lap.len());
                lap.swap(0, i);
            }
        }

        self.queue.extend(lap);

        if self.queue.len() > QUEUE_LIMIT {
            let cut = (self.queue.len() - QUEUE_LIMIT).min(self.pos.unwrap_or(0));
            self.queue.drain(..cut);
            next -= cut;
            self.pos = self.pos.map(|c| c - cut);
        }
        Some(next)
    }

    pub fn upcoming_start(&self) -> usize {
        self.pos.map_or(0, |p| p + 1).min(self.queue.len())
    }

    fn play_at(&mut self, pos: usize) -> bool {
        let Some(&next) = self.queue.get(pos) else {
            return false;
        };

        let Some(audio) = &mut self.audio else {
            return false;
        };

        if !audio.play(&self.songs[next]) {
            return false;
        }

        if self.history.back() != Some(&next) {
            if self.history.len() >= HISTORY_LIMIT {
                self.history.pop_front();
            }
            self.history.push_back(next);
        }
        self.pos = Some(pos);
        true
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match self.mode {
            Mode::Normal => return self.handle_normal(key),
            Mode::Search => self.handle_search(key),
            Mode::Help => self.handle_help(key),
        }
        false
    }

    fn handle_normal(&mut self, key: KeyEvent) -> bool {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.query.is_empty() {
                    return true;
                }
                self.query.clear();
                self.map_state.select(Some(0));
            }

            KeyCode::Up if shift => self.with_audio(|a| a.add_volume(0.05)),
            KeyCode::Down if shift => self.with_audio(|a| a.add_volume(-0.05)),
            KeyCode::Left if shift => self.play_prev(),
            KeyCode::Right if shift => self.play_next(),

            KeyCode::Left => self.seek_backward(),
            KeyCode::Right => self.seek_forward(),

            KeyCode::BackTab => self.tab = self.tab.prev(),
            KeyCode::Tab => self.tab = self.tab.next(),

            KeyCode::Char('r') => self.repeat = !self.repeat,
            KeyCode::Char('s') => self.toggle_shuffle(),
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
                Tab::Maps => self.play_from_maps(),
                Tab::Queue => self.jump_in_queue(),
                _ => {}
            },
            KeyCode::Char(' ') => self.with_audio(|a| a.toggle_pause()),
            _ => {}
        }
        false
    }

    fn handle_search(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.mode = Mode::Normal,
            KeyCode::Esc => {
                self.query.clear();
                self.map_state.select(Some(0));
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.map_state.select(Some(0));
            }
            KeyCode::Char(c) => {
                self.query.push(c);
                self.map_state.select(Some(0));
            }
            _ => {}
        }
    }

    fn handle_help(&mut self, key: KeyEvent) {
        if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
            self.mode = Mode::Normal;
        }
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

    fn play_from_maps(&mut self) {
        if self.is_fading() {
            return;
        }

        let Some(sel) = self.map_state.selected() else {
            return;
        };

        let mut filtered = self.filtered();
        if sel >= filtered.len() {
            return;
        }
        filtered.rotate_left(sel);
        self.original_queue = filtered;

        self.queue.truncate(self.upcoming_start());

        let mut ctx = self.original_queue.clone();
        if self.shuffle {
            fastrand::shuffle(&mut ctx[1..]);
        }

        if self.queue.last() == ctx.first() {
            self.queue.extend_from_slice(&ctx[1..]);
            self.step = Step::Current;
            self.with_audio(|a| a.stop());
        } else {
            self.queue.extend(ctx);
            self.play_next();
        }
    }

    fn jump_in_queue(&mut self) {
        if self.is_fading() {
            return;
        }

        let Some(sel) = self.queue_state.selected() else {
            return;
        };

        let start = self.upcoming_start();
        if start + sel < self.queue.len() {
            self.queue.drain(start..start + sel);
            self.queue_state.select(Some(0));
            self.play_next();
        }
    }

    fn play_next(&mut self) {
        if self.is_fading() {
            return;
        }

        self.step = Step::Next;
        self.with_audio(|a| a.stop());
    }

    fn play_prev(&mut self) {
        if self.is_fading() {
            return;
        }

        let Some(audio) = &mut self.audio else { return };
        if audio.is_empty() {
            self.step = Step::Current;
            return;
        }

        let at_first = !self.pos.is_some_and(|p| p > 0);
        if at_first || audio.position() > RESTART_THRESHOLD {
            audio.seek(Duration::ZERO);
            return;
        }

        self.step = Step::Prev;
        audio.stop();
    }

    pub fn is_fading(&self) -> bool {
        self.audio.as_ref().is_some_and(Audio::is_fading)
    }

    fn toggle_shuffle(&mut self) {
        if self.is_fading() {
            return;
        }

        self.shuffle = !self.shuffle;
        let rest = self.rest_of_original();
        self.queue.truncate(self.upcoming_start());
        self.queue.extend(rest);
    }

    fn rest_of_original(&self) -> Vec<usize> {
        let k = self
            .pos
            .and_then(|p| self.queue.get(p))
            .and_then(|song| self.original_queue.iter().position(|i| i == song))
            .map_or(0, |k| k + 1);

        let mut rest = self.original_queue[k..].to_vec();
        if self.shuffle {
            fastrand::shuffle(&mut rest);
        }

        rest
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
        if self.is_fading() {
            return;
        }

        self.with_audio(|a| a.seek(a.position().saturating_sub(SEEK_STEP)));
    }

    fn seek_forward(&mut self) {
        if self.is_fading() {
            return;
        }

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
    let root = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./songs"));

    let terminal = ratatui::init();
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

    let mut app = App::new(load(&root), picker);

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

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press && app.handle_key(key) {
                return Ok(());
            }
        };
    }
}
