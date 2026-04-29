use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};

use crate::app::App;
use crate::browser::BrowserItem;

pub fn draw(f: &mut Frame, area: Rect, app: &mut App) {
    let title = format!(" Browser · {} ", app.browser.current_dir.display());

    let items: Vec<ListItem> = app
        .browser
        .list
        .entries
        .iter()
        .map(|item| match item {
            BrowserItem::UpDirectory => ListItem::new("..").style(Style::default().fg(Color::DarkGray)),
            BrowserItem::Entry(path) => {
                let label = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
                    .to_string();
                let suffix = if path.is_dir() {
                    "/"
                } else if crate::library::is_audio_file(path) {
                    "  ♪"
                } else {
                    ""
                };
                let style = if path.is_dir() {
                    Style::default().fg(Color::Cyan)
                } else if crate::library::is_audio_file(path) {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::Gray)
                };
                ListItem::new(format!("{label}{suffix}")).style(style)
            }
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut app.browser.list.state);
}
