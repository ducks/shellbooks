use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{App, View};

mod book_detail;
mod bookmarks;
mod browser_view;
mod library_view;
mod now_playing;

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // tab bar
            Constraint::Min(0),     // body
            Constraint::Length(1),  // status
        ])
        .split(f.area());

    draw_tabs(f, chunks[0], app);

    match app.view {
        View::Library => library_view::draw(f, chunks[1], app),
        View::Browser => browser_view::draw(f, chunks[1], app),
        View::BookDetail => book_detail::draw(f, chunks[1], app),
        View::NowPlaying => now_playing::draw(f, chunks[1], app),
        View::Bookmarks => bookmarks::draw(f, chunks[1], app),
    }

    draw_status(f, chunks[2], app);
}

fn draw_tabs(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let tabs = [
        ("library", View::Library),
        ("browser", View::Browser),
        ("book", View::BookDetail),
        ("now playing", View::NowPlaying),
        ("bookmarks", View::Bookmarks),
    ];
    let mut spans = vec![];
    for (i, (label, view)) in tabs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let style = if app.view == *view {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(*label, style));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_status(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let speed = format!("{:.2}x", app.player.speed);
    let state = match app.player.state {
        crate::audio::PlayerState::Playing => "▶",
        crate::audio::PlayerState::Paused => "⏸",
        crate::audio::PlayerState::Idle => "·",
    };

    let help = match app.view {
        View::Library => "q quit · a add · j/k move · tab view",
        View::Browser => "q quit · enter open/import · h up · esc back · tab view",
        _ => "q quit · space play/pause · +/- speed · tab view",
    };

    let mut spans = vec![
        Span::raw("  "),
        Span::styled(state, Style::default().fg(Color::Yellow)),
        Span::raw("  speed "),
        Span::styled(speed, Style::default().fg(Color::Cyan)),
        Span::raw("    "),
    ];
    if let Some(msg) = &app.status {
        spans.push(Span::styled(
            msg.clone(),
            Style::default().fg(Color::Green),
        ));
    } else {
        spans.push(Span::raw(help));
    }

    f.render_widget(
        Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::TOP)),
        area,
    );
}
