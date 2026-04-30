use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = if app.library.books.is_empty() {
        vec![ListItem::new(
            "no books yet — add a path to ~/.config/shellbooks/config.toml",
        )]
    } else {
        app.library
            .books
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let style = if i == app.selected_book {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let author = b.author.as_deref().unwrap_or("—");
                ListItem::new(format!("{}  ·  {}", b.title, author)).style(style)
            })
            .collect()
    };
    f.render_widget(
        List::new(items).block(Block::default().borders(Borders::ALL).title("Library")),
        area,
    );
}
