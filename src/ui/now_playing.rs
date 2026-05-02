use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &mut App) {
    let book = app.library.books.get(app.selected_book);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(area);

    // Header: title / author / current chapter
    let mut header_lines: Vec<Line> = Vec::new();
    if let Some(b) = book {
        header_lines.push(Line::from(vec![Span::styled(
            b.title.clone(),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )]));
        if let Some(author) = &b.author {
            header_lines.push(Line::from(format!("by {author}")));
        }
        let chapter_label = b
            .chapters
            .get(b.progress.current_chapter)
            .map(|c| format!("Chapter: {}", c.title))
            .unwrap_or_else(|| "Chapter: —".into());
        header_lines.push(Line::from(chapter_label));
    } else {
        header_lines.push(Line::from("Nothing playing"));
    }
    f.render_widget(
        Paragraph::new(header_lines)
            .block(Block::default().borders(Borders::ALL).title("Now Playing")),
        chunks[0],
    );

    // Progress gauge
    let total = book.map(|b| b.total_duration).unwrap_or_default();
    let pos = app.player.position;
    let pct = if total.as_secs() == 0 {
        0
    } else {
        ((pos.as_secs_f64() / total.as_secs_f64()) * 100.0).clamp(0.0, 100.0) as u16
    };
    let label = format!("{} / {}", fmt_duration(pos), fmt_duration(total));
    f.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL))
            .gauge_style(Style::default().fg(Color::Yellow))
            .percent(pct)
            .label(label),
        chunks[1],
    );

    // Footer: keybinds reminder
    let footer = Paragraph::new(
        "space play/pause   ,/. ±10s   [/] ±60s   +/- speed   tab view"
    )
    .style(Style::default().fg(Color::DarkGray))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn fmt_duration(d: std::time::Duration) -> String {
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
