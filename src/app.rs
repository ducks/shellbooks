use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

use crate::audio::Player;
use crate::config::Config;
use crate::library::Library;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Library,
    BookDetail,
    NowPlaying,
    Bookmarks,
}

pub struct App {
    pub cfg: Config,
    pub library: Library,
    pub player: Player,
    pub view: View,
    pub selected_book: usize,
    pub selected_chapter: usize,
    pub should_quit: bool,
}

impl App {
    pub fn new(cfg: Config) -> Result<Self> {
        let player = Player::new(cfg.default_speed)?;
        let library = Library::default();
        Ok(Self {
            cfg,
            library,
            player,
            view: View::Library,
            selected_book: 0,
            selected_chapter: 0,
            should_quit: false,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        // TODO: load library.json, then library.scan(self.cfg.library_paths)
        // before entering the event loop.

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let mut term = Terminal::new(CrosstermBackend::new(stdout))?;

        let result = self.event_loop(&mut term);

        disable_raw_mode()?;
        execute!(term.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
        term.show_cursor()?;

        result
    }

    fn event_loop<B: ratatui::backend::Backend>(
        &mut self,
        term: &mut Terminal<B>,
    ) -> Result<()> {
        loop {
            term.draw(|f| crate::ui::draw(f, self))?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    self.handle_key(key.code);
                }
            }

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char(' ') => self.player.toggle_pause(),
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.player.set_speed(self.player.speed + 0.05);
            }
            KeyCode::Char('-') => {
                self.player.set_speed(self.player.speed - 0.05);
            }
            KeyCode::Tab => {
                self.view = match self.view {
                    View::Library => View::BookDetail,
                    View::BookDetail => View::NowPlaying,
                    View::NowPlaying => View::Bookmarks,
                    View::Bookmarks => View::Library,
                };
            }
            _ => {}
        }
    }
}
