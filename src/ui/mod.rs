use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};
use std::time::Duration;

use crate::app::{App, View};

mod bookmarks;
mod browser_view;
mod info_modal;
mod library_view;

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab bar
            Constraint::Min(0),    // body
            Constraint::Length(3), // playback footer (gauge + meta line)
            Constraint::Length(1), // help/status line
        ])
        .split(f.area());

    draw_tabs(f, chunks[0], app);

    match app.view {
        View::Library => library_view::draw(f, chunks[1], app),
        View::Bookmarks => bookmarks::draw(f, chunks[1], app),
        View::Browser => browser_view::draw(f, chunks[1], app),
    }

    draw_playback_footer(f, chunks[2], app);
    draw_status(f, chunks[3], app);

    if app.show_info {
        info_modal::draw(f, app);
    }
}

fn draw_tabs(f: &mut Frame, area: Rect, app: &App) {
    let tabs = [
        ('1', "library", View::Library),
        ('2', "bookmarks", View::Bookmarks),
        ('3', "browser", View::Browser),
    ];
    let mut spans = vec![];
    for (i, (digit, label, view)) in tabs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let active = app.view == *view;
        let digit_style = if active {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };
        let label_style = if active {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(digit.to_string(), digit_style));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(*label, label_style));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Persistent footer with playback state — book title, chapter, position,
/// duration, speed, and a thin progress gauge. Always visible.
fn draw_playback_footer(f: &mut Frame, area: Rect, app: &App) {
    let book = app.library.books.get(app.selected_book);

    let (title, chapter_label) = match book {
        Some(b) => {
            let chapter = b
                .chapters
                .iter()
                .find(|c| c.file_index == app.player.queue_index)
                .map(|c| c.title.as_str())
                .unwrap_or("—");
            (b.title.as_str(), chapter)
        }
        None => ("Nothing playing", "—"),
    };

    let total = book.map(|b| b.total_duration).unwrap_or_default();
    let pos = app.player.position;
    let pct = if total.as_secs() == 0 {
        0
    } else {
        ((pos.as_secs_f64() / total.as_secs_f64()) * 100.0).clamp(0.0, 100.0) as u16
    };

    let state_glyph = match app.player.state {
        crate::audio::PlayerState::Playing => "▶",
        crate::audio::PlayerState::Paused => "⏸",
        crate::audio::PlayerState::Idle => "·",
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(2)])
        .split(area);

    let meta = Line::from(vec![
        Span::styled(state_glyph, Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::styled(title, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("  ·  "),
        Span::styled(chapter_label, Style::default().fg(Color::Cyan)),
        Span::raw("  ·  "),
        Span::raw(format!("{} / {}", fmt_duration(pos), fmt_duration(total))),
        Span::raw("  ·  "),
        Span::styled(
            format!("{:.2}x", app.player.speed),
            Style::default().fg(Color::Magenta),
        ),
    ]);
    f.render_widget(Paragraph::new(meta), chunks[0]);

    f.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::TOP))
            .gauge_style(Style::default().fg(Color::Yellow))
            .percent(pct)
            .label(""),
        chunks[1],
    );
}

fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    let help = match app.view {
        View::Library => {
            "1-3 jump · tab focus · enter play · a add · d delete · i info · q quit"
        }
        View::Browser => "1-3 jump · a import · enter descend · h up · esc back · q quit",
        View::Bookmarks => "1-3 jump · enter jump · i info · q quit",
    };
    let mut spans = vec![Span::raw("  ")];
    if let Some(msg) = &app.status {
        spans.push(Span::styled(msg.clone(), Style::default().fg(Color::Green)));
    } else {
        spans.push(Span::styled(help, Style::default().fg(Color::DarkGray)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn fmt_duration(d: Duration) -> String {
    let total = d.as_secs();
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}
