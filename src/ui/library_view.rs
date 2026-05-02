use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::app::{App, LibraryFocus};

pub fn draw(f: &mut Frame, area: Rect, app: &mut App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    draw_books_pane(f, cols[0], app);
    draw_chapters_pane(f, cols[1], app);
}

fn draw_books_pane(f: &mut Frame, area: Rect, app: &App) {
    let active = app.library_focus == LibraryFocus::Books;
    let title = title_with_focus(" Books ", active, app.library.books.len());

    let items: Vec<ListItem> = if app.library.books.is_empty() {
        vec![ListItem::new(
            "no books yet — press 3 then a to add one",
        )]
    } else {
        app.library
            .books
            .iter()
            .map(|b| {
                let author = b.author.as_deref().unwrap_or("—");
                ListItem::new(format!("{}  ·  {}", b.title, author))
            })
            .collect()
    };

    let mut state = ListState::default();
    if !app.library.books.is_empty() {
        state.select(Some(app.selected_book));
    }

    let list = List::new(items)
        .block(focus_block(active).title(title))
        .highlight_style(highlight_style(active))
        .highlight_symbol(if active { "▶ " } else { "  " });

    f.render_stateful_widget(list, area, &mut state);
}

fn draw_chapters_pane(f: &mut Frame, area: Rect, app: &App) {
    let active = app.library_focus == LibraryFocus::Chapters;
    let book = app.library.books.get(app.selected_book);
    let chapter_count = book.map(|b| b.chapters.len()).unwrap_or(0);
    let title = title_with_focus(" Chapters ", active, chapter_count);

    let items: Vec<ListItem> = match book {
        Some(b) if !b.chapters.is_empty() => b
            .chapters
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let is_current = i == b.progress.current_chapter;
                let prefix = if is_current { "●" } else { " " };
                let label = format!("{prefix} {}", c.title);
                let style = if is_current {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                };
                ListItem::new(label).style(style)
            })
            .collect(),
        Some(_) => vec![ListItem::new("(no chapters detected)")
            .style(Style::default().fg(Color::DarkGray))],
        None => vec![ListItem::new("select a book to see its chapters")
            .style(Style::default().fg(Color::DarkGray))],
    };

    let mut state = ListState::default();
    if chapter_count > 0 {
        state.select(Some(app.selected_chapter.min(chapter_count - 1)));
    }

    let list = List::new(items)
        .block(focus_block(active).title(title))
        .highlight_style(highlight_style(active))
        .highlight_symbol(if active { "▶ " } else { "  " });

    f.render_stateful_widget(list, area, &mut state);
}

fn focus_block(active: bool) -> Block<'static> {
    let border_style = if active {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Block::default().borders(Borders::ALL).border_style(border_style)
}

fn title_with_focus(label: &'static str, active: bool, count: usize) -> String {
    if active {
        format!("{label}({count}) [active] ")
    } else {
        format!("{label}({count}) ")
    }
}

fn highlight_style(active: bool) -> Style {
    if active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    }
}
