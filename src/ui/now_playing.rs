use ratatui::{
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, area: Rect, _app: &App) {
    f.render_widget(
        Paragraph::new("now playing (TODO: progress bar, chapter title, sleep timer)")
            .block(Block::default().borders(Borders::ALL).title("Now Playing")),
        area,
    );
}
