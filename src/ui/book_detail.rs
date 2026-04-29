use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, area: Rect, _app: &App) {
    f.render_widget(
        Paragraph::new("book detail (TODO: cover + chapters + metadata)")
            .block(Block::default().borders(Borders::ALL).title("Book")),
        area,
    );
}
