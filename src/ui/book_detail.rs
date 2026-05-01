use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &mut App) {
    let Some(book) = app.library.books.get(app.selected_book) else {
        f.render_widget(
            Paragraph::new("no book selected — go to Library and pick one")
                .block(Block::default().borders(Borders::ALL).title("Book")),
            area,
        );
        return;
    };

    // Two columns: metadata on the left, chapter list on the right.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let mut lines: Vec<Line> = vec![
        Line::from(vec![Span::styled(
            book.title.clone(),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
    ];
    if let Some(author) = &book.author {
        lines.push(Line::from(format!("by {author}")));
    }
    if let Some(narrator) = &book.narrator {
        lines.push(Line::from(format!("read by {narrator}")));
    }
    if let Some(series) = &book.series {
        let suffix = book
            .series_index
            .map(|i| format!(" #{i}"))
            .unwrap_or_default();
        lines.push(Line::from(format!("{series}{suffix}")));
    }
    if let Some(year) = book.year {
        lines.push(Line::from(format!("{year}")));
    }
    lines.push(Line::from(""));
    if let Some(cover) = &book.cover_path {
        lines.push(Line::from(Span::styled(
            format!("cover: {}", cover.display()),
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines.push(Line::from(Span::styled(
        format!("id {}", book.id),
        Style::default().fg(Color::DarkGray),
    )));

    f.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title("Book"))
            .wrap(ratatui::widgets::Wrap { trim: true }),
        cols[0],
    );

    let chapter_items: Vec<ListItem> = if book.chapters.is_empty() {
        vec![ListItem::new("(chapters not yet detected)")
            .style(Style::default().fg(Color::DarkGray))]
    } else {
        book.chapters
            .iter()
            .map(|c| ListItem::new(c.title.clone()))
            .collect()
    };
    f.render_widget(
        List::new(chapter_items)
            .block(Block::default().borders(Borders::ALL).title("Chapters")),
        cols[1],
    );
}
