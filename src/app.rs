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
use crate::library::{BookKind, Library};

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

    /// Start playing the currently-selected library book from its saved
    /// progress. Quietly does nothing if no books are loaded.
    fn play_selected(&mut self) {
        let Some(book) = self.library.books.get(self.selected_book) else {
            return;
        };
        let queue: Vec<std::path::PathBuf> = match &book.kind {
            BookKind::SingleFile { path } => vec![path.clone()],
            BookKind::MultiFile { files } => files.clone(),
        };

        let resume_chapter = book.progress.current_chapter;
        let resume_offset = book.progress.position;
        let queue_index = book
            .chapters
            .get(resume_chapter)
            .map(|c| c.file_index)
            .unwrap_or(0);

        match self.player.play(queue, queue_index, resume_offset) {
            Ok(()) => {
                self.view = View::NowPlaying;
                self.status = Some(format!("playing: {}", book.title));
            }
            Err(err) => {
                self.status = Some(format!("playback failed: {err}"));
            }
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
            self.player.tick();
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
                self.persist_progress();
                break;
            }
        }
        Ok(())
    }

    /// Save the current player position back to the active book and
    /// flush library.json. Called on quit and could be called periodically
    /// in a future revision.
    fn persist_progress(&mut self) {
        if self.player.queue.is_empty() {
            return;
        }
        if let Some(book) = self.library.books.get_mut(self.selected_book) {
            book.progress.position = self.player.position;
            book.progress.current_chapter = book
                .chapters
                .iter()
                .position(|c| c.file_index == self.player.queue_index)
                .unwrap_or(0);
        }
        self.save_library();
    }

    fn handle_key(&mut self, code: KeyCode) {
        // Clear any one-shot status line on the next keypress.
        self.status = None;

        match code {
            // ---- Global ----
            KeyCode::Char('q') => self.should_quit = true,

            // Direct screen jumps (cmus-style, like shelltrax).
            KeyCode::Char('1') => self.view = View::Library,
            KeyCode::Char('2') => self.view = View::BookDetail,
            KeyCode::Char('3') => self.view = View::NowPlaying,
            KeyCode::Char('4') => self.view = View::Bookmarks,
            KeyCode::Char('5') => self.view = View::Browser,

            // Tab still cycles for users who prefer it.
            KeyCode::Tab => {
                self.view = match self.view {
                    View::Library => View::BookDetail,
                    View::BookDetail => View::NowPlaying,
                    View::NowPlaying => View::Bookmarks,
                    View::Bookmarks => View::Browser,
                    View::Browser => View::Library,
                };
            }

            // Playback controls work regardless of view.
            KeyCode::Char(' ') | KeyCode::Char('c') => self.player.toggle_pause(),
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.player.set_speed(self.player.speed + 0.05);
            }
            KeyCode::Char('-') => {
                self.player.set_speed(self.player.speed - 0.05);
            }
            KeyCode::Char(',') if self.view != View::Browser => {
                let _ = self.player.seek(Duration::from_secs(10), false);
            }
            KeyCode::Char('.') if self.view != View::Browser => {
                let _ = self.player.seek(Duration::from_secs(10), true);
            }
            KeyCode::Char('[') if self.view != View::Browser => {
                let _ = self.player.seek(Duration::from_secs(60), false);
            }
            KeyCode::Char(']') if self.view != View::Browser => {
                let _ = self.player.seek(Duration::from_secs(60), true);
            }

            // ---- Library ----
            KeyCode::Char('a') if self.view == View::Library => {
                self.view = View::Browser;
            }
            KeyCode::Enter if self.view == View::Library => {
                self.play_selected();
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

            // ---- Browser ----
            KeyCode::Char('j') | KeyCode::Down if self.view == View::Browser => {
                self.browser.move_down();
            }
            KeyCode::Char('k') | KeyCode::Up if self.view == View::Browser => {
                self.browser.move_up();
            }
            KeyCode::Char('g') if self.view == View::Browser => self.browser.go_to_top(),
            KeyCode::Char('G') if self.view == View::Browser => self.browser.go_to_bottom(),
            KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace
                if self.view == View::Browser =>
            {
                self.browser.go_up();
            }
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right
                if self.view == View::Browser =>
            {
                // Enter only descends (or returns an audio file path —
                // future use). Never imports. Matches shelltrax.
                let _ = self.browser.open_selected();
            }
            // shelltrax-style import: `a` adds the highlighted path to
            // the library. Works on any directory of audio or a single
            // audio file.
            KeyCode::Char('a') if self.view == View::Browser => {
                if let Some(path) = self.browser.selected_path() {
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
