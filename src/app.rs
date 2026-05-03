use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::{Duration, Instant};

const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(10);

use crate::audio::Player;
use crate::browser::BrowserState;
use crate::config::Config;
use crate::library::{Book, BookKind, Library};

/// Find the chapter index a position belongs to. For multi-file books
/// chapters are 1:1 with files, so queue_index is authoritative. For
/// single-file books all chapters live in file 0 and we walk by start.
pub(crate) fn current_chapter_index(book: &Book, queue_index: usize, position: Duration) -> usize {
    if matches!(book.kind, BookKind::MultiFile { .. }) {
        return book
            .chapters
            .iter()
            .position(|c| c.file_index == queue_index)
            .unwrap_or(0);
    }
    let mut idx = 0;
    for (i, c) in book.chapters.iter().enumerate() {
        if position >= c.start {
            idx = i;
        } else {
            break;
        }
    }
    idx
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Library,
    Bookmarks,
    Browser,
}

/// Which sub-pane has focus inside the Library two-pane layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryFocus {
    Books,
    Chapters,
}

pub struct App {
    pub cfg: Config,
    pub library: Library,
    pub player: Player,
    pub view: View,
    pub selected_book: usize,
    pub selected_chapter: usize,
    pub library_focus: LibraryFocus,
    pub browser: BrowserState,
    /// True when the `i` info modal is overlaid on the current view.
    pub show_info: bool,
    pub should_quit: bool,
    pub status: Option<String>,

    /// ID of the book whose files are currently in the player queue.
    /// `None` when the player is idle. Lets b/z chapter-skip and the
    /// autosave loop know which library entry to update without relying
    /// on selected_book (which can drift as the user navigates).
    pub playing_book_id: Option<String>,
    /// Last time we persisted progress to disk during playback.
    pub last_autosave: Instant,
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
            library_focus: LibraryFocus::Books,
            browser: BrowserState::new(),
            show_info: false,
            should_quit: false,
            status: None,
            playing_book_id: None,
            last_autosave: Instant::now(),
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
    fn play_selected_book(&mut self) {
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

        let book_id = book.id.clone();
        let book_title = book.title.clone();
        match self.player.play(queue, queue_index, resume_offset) {
            Ok(()) => {
                self.playing_book_id = Some(book_id);
                self.last_autosave = Instant::now();
                self.status = Some(format!("playing: {}", book_title));
            }
            Err(err) => {
                self.status = Some(format!("playback failed: {err}"));
            }
        }
    }

    /// Start playing the selected book at the selected chapter.
    fn play_selected_chapter(&mut self) {
        let Some(book) = self.library.books.get(self.selected_book) else {
            return;
        };
        let Some(chapter) = book.chapters.get(self.selected_chapter) else {
            return;
        };
        let queue: Vec<std::path::PathBuf> = match &book.kind {
            BookKind::SingleFile { path } => vec![path.clone()],
            BookKind::MultiFile { files } => files.clone(),
        };
        let book_id = book.id.clone();
        let book_title = book.title.clone();
        let chapter_title = chapter.title.clone();
        let queue_index = chapter.file_index;
        let start = chapter.start;
        match self.player.play(queue, queue_index, start) {
            Ok(()) => {
                self.playing_book_id = Some(book_id);
                self.last_autosave = Instant::now();
                self.status = Some(format!("playing: {} — {}", book_title, chapter_title));
            }
            Err(err) => {
                self.status = Some(format!("playback failed: {err}"));
            }
        }
    }

    /// Skip forward (`forward = true`) or backward to the adjacent chapter
    /// of the *playing* book.
    fn skip_chapter(&mut self, forward: bool) {
        let Some(book_id) = self.playing_book_id.clone() else {
            return;
        };
        let Some(book) = self.library.books.iter().find(|b| b.id == book_id) else {
            return;
        };
        if book.chapters.is_empty() {
            return;
        }

        // Locate the current chapter by current player position. For
        // multi-file books, queue_index identifies the file; for
        // single-file books all chapters share file_index 0 and we
        // disambiguate by the player's current position.
        let current = current_chapter_index(book, self.player.queue_index, self.player.position);

        let next = if forward {
            current + 1
        } else if current == 0 {
            0
        } else {
            current - 1
        };

        let Some(chapter) = book.chapters.get(next) else {
            return;
        };
        let queue: Vec<std::path::PathBuf> = match &book.kind {
            BookKind::SingleFile { path } => vec![path.clone()],
            BookKind::MultiFile { files } => files.clone(),
        };
        let queue_index = chapter.file_index;
        let start = chapter.start;
        let title = chapter.title.clone();
        if let Err(err) = self.player.play(queue, queue_index, start) {
            self.status = Some(format!("seek failed: {err}"));
        } else {
            self.status = Some(format!("chapter: {title}"));
            // Force a save now — chapter changes are a meaningful checkpoint.
            self.persist_progress();
            self.last_autosave = Instant::now();
        }
    }

    /// Remove the highlighted book from the library and persist. If the
    /// removed book was actively playing, stop the player so we don't
    /// keep spinning on a queue whose entries no longer have a backing
    /// catalog entry.
    fn delete_selected_book(&mut self) {
        let Some(removed) = self.library.delete_at(self.selected_book) else {
            return;
        };

        // If the removed book was playing, stop. We compare by checking
        // whether any of its files matches the player's queue.
        let removed_paths: Vec<&std::path::Path> = match &removed.kind {
            crate::library::BookKind::SingleFile { path } => vec![path.as_path()],
            crate::library::BookKind::MultiFile { files } => {
                files.iter().map(|p| p.as_path()).collect()
            }
        };
        if self
            .player
            .queue
            .iter()
            .any(|q| removed_paths.iter().any(|p| *p == q.as_path()))
        {
            self.player.stop();
            self.playing_book_id = None;
        }

        // Keep selected_book valid.
        if self.selected_book >= self.library.books.len() && self.selected_book > 0 {
            self.selected_book -= 1;
        }
        self.selected_chapter = 0;
        self.status = Some(format!("removed: {}", removed.title));
        self.save_library();
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
            self.maybe_autosave();
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

    /// Persist progress every AUTOSAVE_INTERVAL while playing, so a
    /// crash never costs more than ~10 seconds of position data.
    fn maybe_autosave(&mut self) {
        if !matches!(self.player.state, crate::audio::PlayerState::Playing) {
            return;
        }
        if self.last_autosave.elapsed() < AUTOSAVE_INTERVAL {
            return;
        }
        self.persist_progress();
        self.last_autosave = Instant::now();
    }

    /// Save the current player position back to whichever book is actively
    /// playing (tracked by playing_book_id, not selected_book — the user
    /// may have navigated to a different library entry mid-playback).
    /// Called on quit and on the 10s autosave tick.
    fn persist_progress(&mut self) {
        if self.player.queue.is_empty() {
            return;
        }
        let Some(book_id) = self.playing_book_id.clone() else {
            return;
        };
        let queue_index = self.player.queue_index;
        let position = self.player.position;
        if let Some(book) = self.library.books.iter_mut().find(|b| b.id == book_id) {
            book.progress.position = position;
            book.progress.current_chapter = current_chapter_index(book, queue_index, position);
        }
        self.save_library();
    }

    fn handle_key(&mut self, code: KeyCode) {
        // Clear any one-shot status line on the next keypress.
        self.status = None;

        // The info modal swallows most keys: Esc / `i` / `q` close it.
        if self.show_info {
            match code {
                KeyCode::Esc | KeyCode::Char('i') => self.show_info = false,
                KeyCode::Char('q') => self.should_quit = true,
                _ => {}
            }
            return;
        }

        match code {
            // ---- Global ----
            KeyCode::Char('q') => self.should_quit = true,

            // Direct screen jumps (cmus-style, like shelltrax).
            KeyCode::Char('1') => self.view = View::Library,
            KeyCode::Char('2') => self.view = View::Bookmarks,
            KeyCode::Char('3') => self.view = View::Browser,

            // `i` opens the info modal — works in any view.
            KeyCode::Char('i') => self.show_info = true,

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
            // Chapter skip (shelltrax-style: b next, z previous).
            KeyCode::Char('b') if self.view != View::Browser => {
                self.skip_chapter(true);
            }
            KeyCode::Char('z') if self.view != View::Browser => {
                self.skip_chapter(false);
            }

            // ---- Library ----
            KeyCode::Tab if self.view == View::Library => {
                self.library_focus = match self.library_focus {
                    LibraryFocus::Books => LibraryFocus::Chapters,
                    LibraryFocus::Chapters => LibraryFocus::Books,
                };
            }
            KeyCode::Char('a') if self.view == View::Library => {
                self.view = View::Browser;
            }
            KeyCode::Char('d')
                if self.view == View::Library
                    && self.library_focus == LibraryFocus::Books =>
            {
                self.delete_selected_book();
            }
            KeyCode::Enter if self.view == View::Library => match self.library_focus {
                LibraryFocus::Books => self.play_selected_book(),
                LibraryFocus::Chapters => self.play_selected_chapter(),
            },
            KeyCode::Char('j') | KeyCode::Down if self.view == View::Library => {
                self.move_down_in_library();
            }
            KeyCode::Char('k') | KeyCode::Up if self.view == View::Library => {
                self.move_up_in_library();
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
                // Enter only descends. Importing is `a`. Matches shelltrax.
                let _ = self.browser.open_selected();
            }
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

    fn move_down_in_library(&mut self) {
        match self.library_focus {
            LibraryFocus::Books => {
                if !self.library.books.is_empty()
                    && self.selected_book + 1 < self.library.books.len()
                {
                    self.selected_book += 1;
                    self.selected_chapter = 0;
                }
            }
            LibraryFocus::Chapters => {
                let max = self
                    .library
                    .books
                    .get(self.selected_book)
                    .map(|b| b.chapters.len())
                    .unwrap_or(0);
                if max > 0 && self.selected_chapter + 1 < max {
                    self.selected_chapter += 1;
                }
            }
        }
    }

    fn move_up_in_library(&mut self) {
        match self.library_focus {
            LibraryFocus::Books => {
                if self.selected_book > 0 {
                    self.selected_book -= 1;
                    self.selected_chapter = 0;
                }
            }
            LibraryFocus::Chapters => {
                if self.selected_chapter > 0 {
                    self.selected_chapter -= 1;
                }
            }
        }
    }
}
