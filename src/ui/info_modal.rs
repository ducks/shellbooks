use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use std::time::Duration;

use crate::app::App;

/// Centered overlay with full info for the currently-selected book:
/// title, author, narrator, series, year, total duration, chapter
/// count, filesystem path, id. Cover art TODO once we re-add
/// ratatui-image rendering.
pub fn draw(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 70, f.area());
    f.render_widget(Clear, area);

    let book = app.library.books.get(app.selected_book);

    let mut lines: Vec<Line> = Vec::new();
    if let Some(b) = book {
        lines.push(Line::from(vec![Span::styled(
            b.title.clone(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]));
        if let Some(author) = &b.author {
            lines.push(Line::from(vec![
                Span::styled("Author:    ", label_style()),
                Span::raw(author.clone()),
            ]));
        }
        if let Some(narrator) = &b.narrator {
            lines.push(Line::from(vec![
                Span::styled("Narrator:  ", label_style()),
                Span::raw(narrator.clone()),
            ]));
        }
        if let Some(series) = &b.series {
            let suffix = b
                .series_index
                .map(|i| format!(" #{i}"))
                .unwrap_or_default();
            lines.push(Line::from(vec![
                Span::styled("Series:    ", label_style()),
                Span::raw(format!("{series}{suffix}")),
            ]));
        }
        if let Some(year) = b.year {
            lines.push(Line::from(vec![
                Span::styled("Year:      ", label_style()),
                Span::raw(year.to_string()),
            ]));
        }
        lines.push(Line::from(vec![
            Span::styled("Duration:  ", label_style()),
            Span::raw(fmt_duration(b.total_duration)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Chapters:  ", label_style()),
            Span::raw(b.chapters.len().to_string()),
        ]));
        if let Some(cover) = &b.cover_path {
            lines.push(Line::from(vec![
                Span::styled("Cover:     ", label_style()),
                Span::raw(cover.display().to_string()),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Path:      ", label_style()),
            Span::styled(
                b.root.display().to_string(),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("ID:        ", label_style()),
            Span::styled(b.id.clone(), Style::default().fg(Color::DarkGray)),
        ]));
    } else {
        lines.push(Line::from("No book selected"));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "esc / i to close",
        Style::default().fg(Color::DarkGray),
    )));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" Book Info ");

    f.render_widget(
        Paragraph::new(lines).block(block).wrap(Wrap { trim: true }),
        area,
    );
}

fn label_style() -> Style {
    Style::default().fg(Color::Cyan)
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

/// Center a percentage-sized box inside `r`. Standard ratatui modal trick.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
