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
use crate::browser::BrowserState;
use crate::config::Config;
use crate::library::Library;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Library,
    Browser,
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
    pub browser: BrowserState,
    pub should_quit: bool,
    pub status: Option<String>,
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
            browser: BrowserState::new(),
            should_quit: false,
            status: None,
        })
    }

    /// Persist the library to disk. Logged on failure but never fatal —
    /// the app can keep running with an in-memory library.
    fn save_library(&self) {
        if let Some(db_path) = crate::config::library_db_path()
            && let Err(err) = self.library.save(&db_path)
        {
            log::warn!("library save failed: {err}");
        }
    }

    /// Import the path the browser is pointing at. Path may be a directory
    /// of audio files (multi-file book) or a single audio file. Adds it
    /// to the library, persists, and returns the new book's index.
    fn import_from_browser(&mut self, path: std::path::PathBuf) {
        match self.library.import_path(&path) {
            Ok(idx) => {
                self.selected_book = idx;
                self.view = View::Library;
                self.status = Some(format!(
                    "added: {}",
                    self.library.books[idx].title
                ));
                self.save_library();
            }
            Err(err) => {
                self.status = Some(format!("import failed: {err}"));
            }
        }
    }

    pub fn run(&mut self) -> Result<()> {
        // Just load the cached library. New books get in via the explicit
        // file browser (press 'a' from Library) — we don't sweep the disk
        // on launch since not every .mp3 in a tree is an audiobook.
        if let Some(db_path) = crate::config::library_db_path() {
            self.library = crate::library::Library::load(&db_path).unwrap_or_default();
        }

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
            // (`draw` borrows self mutably for ListState; the closure
            // is `FnOnce` so this is fine.)

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
        // Clear any one-shot status line on the next keypress.
        self.status = None;

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
                    View::Library => View::Browser,
                    View::Browser => View::BookDetail,
                    View::BookDetail => View::NowPlaying,
                    View::NowPlaying => View::Bookmarks,
                    View::Bookmarks => View::Library,
                };
            }
            // Library keybinds
            KeyCode::Char('a') if self.view == View::Library => {
                self.view = View::Browser;
            }
            KeyCode::Char('j') | KeyCode::Down if self.view == View::Library => {
                if !self.library.books.is_empty()
                    && self.selected_book + 1 < self.library.books.len()
                {
                    self.selected_book += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up if self.view == View::Library => {
                if self.selected_book > 0 {
                    self.selected_book -= 1;
                }
            }
            // Browser keybinds
            KeyCode::Char('j') | KeyCode::Down if self.view == View::Browser => {
                self.browser.move_down();
            }
            KeyCode::Char('k') | KeyCode::Up if self.view == View::Browser => {
                self.browser.move_up();
            }
            KeyCode::Char('g') if self.view == View::Browser => self.browser.go_to_top(),
            KeyCode::Char('G') if self.view == View::Browser => self.browser.go_to_bottom(),
            KeyCode::Char('h') | KeyCode::Left if self.view == View::Browser => {
                self.browser.go_up();
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right
                if self.view == View::Browser =>
            {
                if let Some(path) = self.browser.open_selected() {
                    self.import_from_browser(path);
                }
            }
            KeyCode::Esc if self.view == View::Browser => {
                self.view = View::Library;
            }
            _ => {}
        }
    }
}
