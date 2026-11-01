use std::time::Duration;

use ratatui::{
    Frame,
    layout::{
        Constraint::{self},
        Layout, Rect,
    },
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, LineGauge, List, Padding, Paragraph, Row, Table, Tabs},
};
use ratatui_image::Image;

use crate::{App, Mode, Tab, beatmap::OsuMap, thumbnail};

const KEYMAP: &[(&str, &str)] = &[
    ("/", "Search"),
    ("?", "Show Help"),
    ("q / Esc", "Quit"),
    ("k / ↑", "Select Up"),
    ("j / ↓", "Select Down"),
    ("Shift + ↑", "Volume Up"),
    ("Shift + ↓", "Volume Down"),
    ("Shift + →", "Next Song"),
    ("Shift + ←", "Prev Songwn"),
    ("→", "Seek +5s"),
    ("←", "Seek -5s"),
    ("Space", "Toggle Pause"),
    ("r", "Toggle Repeat"),
    ("s", "Toggle Shuffle"),
    ("Enter", "Play selected"),
    ("PageUp", "Scroll PageUp"),
    ("PageDown", "Scroll Pagedown"),
    ("Home", "Scroll Top"),
    ("End", "Scroll Last "),
    ("Tab", "Switch Tabs"),
];

pub fn draw(frame: &mut Frame, app: &mut App) {
    let show_search = matches!(app.mode, Mode::Search);
    let show_playing = !show_search && app.playing().is_some();

    let show_thumbnail = frame.area().height >= 24 && !matches!(app.mode, Mode::Help);
    let playing_height = if show_thumbnail {
        thumbnail::ROWS + 2
    } else {
        4
    };

    let [tabs_area, main_area, search_area, playing_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(if show_search { 3 } else { 0 }),
        Constraint::Length(if show_playing { playing_height } else { 0 }),
    ])
    .areas(frame.area());
    app.page_height = main_area.height.saturating_sub(2);

    draw_tabs(frame, tabs_area, app);

    match app.tab {
        Tab::Maps => draw_maps(frame, main_area, app),
        Tab::Queue => draw_queue(frame, main_area, app),
        Tab::History => draw_history(frame, main_area, app),
    }

    if show_search {
        draw_search(frame, search_area, app);
    }

    if show_playing {
        draw_now_playing(frame, playing_area, app, show_thumbnail);
    }

    if matches!(app.mode, Mode::Help) {
        draw_help(frame);
    }
}

fn label(m: &OsuMap) -> String {
    format!(
        "{} / {}",
        m.metadata.display_title(),
        m.metadata.display_artist()
    )
}

fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let tabs = Tabs::new(Tab::ALL.map(Tab::title))
        .select(app.tab as usize)
        .highlight_style(Style::new().bg(Color::Cyan).add_modifier(Modifier::BOLD));
    frame.render_widget(tabs, area);
}

fn draw_maps(frame: &mut Frame, area: Rect, app: &mut App) {
    let filtered = &app.filtered;
    let header = if app.query.is_empty() {
        format!("All ({})", filtered.len())
    } else {
        format!("Search: {} ({})", app.query, filtered.len())
    };

    let list = List::new(filtered.iter().map(|&i| {
        let is_playing = app.playing() == Some(i);
        let mark = if is_playing { "♪ " } else { "  " };
        let line = Line::from(format!("{}{}", mark, label(&app.songs[i])));
        if is_playing {
            line.style(Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        } else {
            line
        }
    }))
    .block(
        Block::bordered()
            .title(format!(" Maps {} ", header))
            .title_bottom(Line::from(" ?: help ").right_aligned()),
    )
    .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, area, &mut app.map_state);
}

fn draw_queue(frame: &mut Frame, area: Rect, app: &mut App) {
    let upcoming = app.playlist.upcoming();
    let list = List::new(
        upcoming
            .iter()
            .map(|&i| Line::from(format!(" {}", label(&app.songs[i])))),
    )
    .block(
        Block::bordered()
            .title(format!(" Queue ({}) ", upcoming.len()))
            .title_bottom(Line::from(" ?: Help ").right_aligned()),
    )
    .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, area, &mut app.queue_state);
}

fn draw_history(frame: &mut Frame, area: Rect, app: &mut App) {
    let history = app.playlist.history();
    let list = List::new(
        history
            .iter()
            .rev()
            .map(|&i| Line::from(format!(" {}", label(&app.songs[i])))),
    )
    .block(
        Block::bordered()
            .title(format!(" History ({}) ", history.len()))
            .title_bottom(Line::from(" ?: Help ").right_aligned()),
    )
    .highlight_style(Style::new().add_modifier(Modifier::REVERSED));
    frame.render_stateful_widget(list, area, &mut app.history_state);
}

fn draw_search(frame: &mut Frame, area: Rect, app: &App) {
    frame.render_widget(
        Paragraph::new(format!("/{}_", app.query)).block(Block::bordered().title(" Search ")),
        area,
    );
}

fn status_span(name: &str, on: bool) -> Span<'_> {
    let style = if on {
        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::new().fg(Color::DarkGray)
    };
    Span::styled(name, style)
}

fn draw_now_playing(frame: &mut Frame, area: Rect, app: &App, show_thumbnail: bool) {
    let Some(i) = app.playing() else { return };
    let Some(audio) = &app.audio else { return };

    let [song_area, status_area] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(10)]).areas(area);

    let song_block = Block::bordered().title(" Now Playing ");
    let song_inner = song_block.inner(song_area);
    frame.render_widget(song_block, song_area);

    let thumb_width = if show_thumbnail {
        app.thumbnail.cols() + 1
    } else {
        0
    };
    let [_, thumb_area, meta_area] = Layout::horizontal([
        Constraint::Length(1),
        Constraint::Length(thumb_width),
        Constraint::Fill(1),
    ])
    .areas(song_inner);

    if show_thumbnail {
        draw_thumbnail(frame, thumb_area, app);
    }

    let [title_area, seek_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(meta_area);

    frame.render_widget(
        Paragraph::new(format!("{}", label(&app.songs[i]))),
        title_area,
    );

    draw_seekbar(frame, seek_area, audio.position(), audio.duration());

    let status = vec![
        Line::from(vec![
            Span::raw(format!(" {}", if audio.is_puased() { "⏸" } else { "▶" })),
            Span::raw(" "),
            status_span("S", app.playlist.shuffle()),
            Span::raw(" "),
            status_span("R", app.playlist.repeat()),
        ]),
        Line::from(vec![Span::raw(format!(" v:{}%", app.volume()))]),
    ];

    frame.render_widget(
        Paragraph::new(status).block(Block::bordered().title(" Status ")),
        status_area,
    );
}

fn draw_help(frame: &mut Frame) {
    let rows = KEYMAP.iter().map(|(key, desc)| Row::new([*key, *desc]));
    let table = Table::new(rows, [Constraint::Length(10), Constraint::Fill(1)]).block(
        Block::bordered()
            .title(" Help ")
            .title_bottom(Line::from(" q / Esc to close help ").right_aligned())
            .padding(Padding::symmetric(2, 1)),
    );
    let popup = frame.area().centered(
        Constraint::Length(40),
        Constraint::Length(KEYMAP.len() as u16 + 4),
    );
    frame.render_widget(Clear, popup);
    frame.render_widget(table, popup);
}

fn draw_thumbnail(frame: &mut Frame, area: Rect, app: &App) {
    if let Some(image) = &app.thumbnail.image {
        frame.render_widget(Image::new(image), area);
    }
}

fn format_time(time: Duration) -> String {
    let secs = time.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

fn draw_seekbar(frame: &mut Frame, area: Rect, position: Duration, duration: Option<Duration>) {
    let [bar_area, _] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(1)]).areas(area);

    let Some(duration) = duration.filter(|d| !d.is_zero()) else {
        frame.render_widget(Paragraph::new(format_time(position)), bar_area);
        return;
    };

    let ratio = (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0);
    let gauge = LineGauge::default()
        .ratio(ratio)
        .label(format!(
            "{} / {}",
            format_time(position),
            format_time(duration)
        ))
        .filled_style(Style::new().fg(Color::Cyan))
        .unfilled_style(Style::new().fg(Color::DarkGray));
    frame.render_widget(gauge, bar_area);
}
