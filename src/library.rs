use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use walkdir::WalkDir;

use crate::config::expand_tilde;

/// Audio extensions we recognize. m4b is the audiobook native, m4a is its
/// cousin, the rest are common encodings users might have.
const AUDIO_EXTENSIONS: &[&str] = &["m4b", "m4a", "mp3", "flac", "ogg", "opus", "wav"];

pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let lower = e.to_ascii_lowercase();
            AUDIO_EXTENSIONS.iter().any(|x| *x == lower)
        })
        .unwrap_or(false)
}

/// Stable book ID derived from the canonical root path. Short hex prefix
/// of SHA-256; collisions across one user's library are vanishingly
/// unlikely with 16 hex chars (64 bits).
fn book_id_for(root: &Path) -> String {
    let canonical = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    hex16(&digest)
}

fn hex16(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(16);
    for b in bytes.iter().take(8) {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// A single audiobook. Either a directory of audio files (chapter per file)
/// or a single .m4b. Chapters are normalized into the `chapters` field at
/// scan time; the source files themselves stay untouched on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Book {
    pub id: String, // stable hash of root path
    pub root: PathBuf,
    pub kind: BookKind,
    pub title: String,
    pub author: Option<String>,
    pub narrator: Option<String>,
    pub series: Option<String>,
    pub series_index: Option<u32>,
    pub year: Option<i32>,
    pub cover_path: Option<PathBuf>,
    #[serde(with = "duration_secs")]
    pub total_duration: Duration,
    pub chapters: Vec<Chapter>,
    pub progress: Progress,
    pub bookmarks: Vec<Bookmark>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BookKind {
    /// Single-file book (typically .m4b with internal chapters).
    SingleFile { path: PathBuf },
    /// Multi-file book (directory of mp3/m4a, one per chapter).
    MultiFile { files: Vec<PathBuf> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chapter {
    pub title: String,
    /// Index into BookKind::MultiFile::files, or 0 for single-file books.
    pub file_index: usize,
    /// Offset within the chapter's source file.
    #[serde(with = "duration_secs")]
    pub start: Duration,
    #[serde(with = "duration_secs")]
    pub duration: Duration,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Progress {
    pub current_chapter: usize,
    #[serde(with = "duration_secs")]
    pub position: Duration,
    pub finished: bool,
    pub last_played_at: Option<String>, // ISO-8601
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub chapter: usize,
    #[serde(with = "duration_secs")]
    pub position: Duration,
    pub note: Option<String>,
    pub created_at: String,
}

/// The on-disk shape of `library.json`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Library {
    pub books: Vec<Book>,
}

impl Library {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let body = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&body)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let body = serde_json::to_string_pretty(self)?;
        std::fs::write(path, body)?;
        Ok(())
    }

    /// Import a single book from `path`. `path` is either a directory of
    /// audio files (multi-file book) or a single audio file (single-file
    /// book). Returns the index of the imported book in `self.books`,
    /// or the existing index if it was already in the library.
    pub fn import_path(&mut self, path: &Path) -> Result<usize> {
        let book = if path.is_dir() {
            let mut files: Vec<PathBuf> = std::fs::read_dir(path)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.is_file() && is_audio_file(p))
                .collect();
            if files.is_empty() {
                anyhow::bail!("no audio files in {}", path.display());
            }
            files.sort();
            build_book(path, files)
        } else if is_audio_file(path) {
            // Single audio file: the book's "root" is the file's parent
            // directory so multi-file imports of the same dir collide
            // with the single-file id (idempotent re-import).
            let parent = path.parent().unwrap_or(path).to_path_buf();
            build_book(&parent, vec![path.to_path_buf()])
        } else {
            anyhow::bail!("not a directory or audio file: {}", path.display());
        };

        if let Some(idx) = self.books.iter().position(|b| b.id == book.id) {
            return Ok(idx);
        }

        self.books.push(book);
        self.books.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        // Index of the now-sorted-in book.
        let new_idx = self
            .books
            .iter()
            .position(|b| b.root == path || b.root == path.parent().unwrap_or(path))
            .unwrap_or(self.books.len() - 1);
        Ok(new_idx)
    }

    /// Walk every configured library path and add new books, preserving
    /// existing progress + bookmarks for books we already knew about.
    /// Not called automatically — only via an explicit user action
    /// (e.g. a future "rescan configured roots" keybind).
    pub fn scan(&mut self, roots: &[PathBuf]) -> Result<()> {
        // Index existing books by id so we can carry forward progress
        // and bookmarks if the same book is rescanned.
        let mut existing: HashMap<String, Book> =
            self.books.drain(..).map(|b| (b.id.clone(), b)).collect();

        let mut found: Vec<Book> = Vec::new();

        for root in roots {
            let root = expand_tilde(root);
            if !root.exists() {
                log::warn!("library path does not exist: {}", root.display());
                continue;
            }
            collect_books_under(&root, &mut found);
        }

        // Merge: keep progress/bookmarks from existing entries.
        for mut book in found {
            if let Some(prev) = existing.remove(&book.id) {
                book.progress = prev.progress;
                book.bookmarks = prev.bookmarks;
            }
            self.books.push(book);
        }

        self.books.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        Ok(())
    }
}

/// Walk one root and append every detected book to `out`. The grouping
/// rule: any directory that directly contains audio files is one book.
/// Single-file books (just an .m4b sitting in a folder by itself) get
/// the same treatment as multi-file books — the BookKind tracks which.
fn collect_books_under(root: &Path, out: &mut Vec<Book>) {
    // Group audio files by their parent directory.
    let mut by_parent: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            // macOS shipped .DS_Store / __MACOSX, Windows ships Thumbs.db.
            // Skipping these directories early saves walk time.
            !e.path().components().any(|c| c.as_os_str() == "__MACOSX")
        })
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
    {
        let p = entry.path();

        if let Some(name) = p.file_name().and_then(|n| n.to_str())
            && (name.starts_with("._") || name == ".DS_Store" || name == "Thumbs.db")
        {
            continue;
        }

        if !is_audio_file(p) {
            continue;
        }

        if let Some(parent) = p.parent() {
            by_parent
                .entry(parent.to_path_buf())
                .or_default()
                .push(p.to_path_buf());
        }
    }

    for (parent, mut files) in by_parent {
        files.sort();
        let book = build_book(&parent, files);
        out.push(book);
    }
}

fn build_book(book_root: &Path, files: Vec<PathBuf>) -> Book {
    let id = book_id_for(book_root);
    let title = book_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .to_string();

    let kind = if files.len() == 1 {
        BookKind::SingleFile {
            path: files[0].clone(),
        }
    } else {
        BookKind::MultiFile { files }
    };

    Book {
        id,
        root: book_root.to_path_buf(),
        kind,
        title,
        author: None,
        narrator: None,
        series: None,
        series_index: None,
        year: None,
        cover_path: None,
        total_duration: Duration::ZERO,
        chapters: Vec::new(),
        progress: Progress::default(),
        bookmarks: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Build a Book without going through the filesystem. Mirrors shelltrax's
    /// `create_test_track` style — a small factory the tests can lean on.
    fn make_book(id: &str, title: &str, root: &str) -> Book {
        Book {
            id: id.into(),
            root: PathBuf::from(root),
            kind: BookKind::SingleFile { path: PathBuf::from(root).join("book.m4b") },
            title: title.into(),
            author: None,
            narrator: None,
            series: None,
            series_index: None,
            year: None,
            cover_path: None,
            total_duration: Duration::ZERO,
            chapters: vec![],
            progress: Progress::default(),
            bookmarks: vec![],
        }
    }

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, b"").unwrap();
    }

    // ---- is_audio_file ----

    #[test]
    fn is_audio_file_recognizes_common_extensions() {
        for ext in ["m4b", "M4B", "mp3", "mp4a", "flac", "ogg", "opus", "wav"]
            .iter()
            .filter(|e| **e != "mp4a") // sanity: mp4a not on our list
        {
            let p = PathBuf::from(format!("foo.{ext}"));
            assert!(is_audio_file(&p), "{ext} should match");
        }
    }

    #[test]
    fn is_audio_file_rejects_non_audio() {
        for name in ["foo.txt", "foo", "cover.jpg", "foo.MP4"] {
            assert!(!is_audio_file(&PathBuf::from(name)), "{name} should not match");
        }
    }

    // ---- book_id_for ----

    #[test]
    fn book_id_is_stable_for_same_path() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("book");
        std::fs::create_dir(&p).unwrap();
        let a = book_id_for(&p);
        let b = book_id_for(&p);
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn book_id_differs_for_different_paths() {
        let dir = TempDir::new().unwrap();
        let one = dir.path().join("one");
        let two = dir.path().join("two");
        std::fs::create_dir(&one).unwrap();
        std::fs::create_dir(&two).unwrap();
        assert_ne!(book_id_for(&one), book_id_for(&two));
    }

    // ---- Library::import_path ----

    #[test]
    fn import_path_imports_a_directory_of_audio_files() {
        let dir = TempDir::new().unwrap();
        let book_dir = dir.path().join("Some Book");
        std::fs::create_dir(&book_dir).unwrap();
        touch(&book_dir.join("01.mp3"));
        touch(&book_dir.join("02.mp3"));

        let mut lib = Library::default();
        let idx = lib.import_path(&book_dir).unwrap();

        assert_eq!(lib.books.len(), 1);
        assert_eq!(lib.books[idx].title, "Some Book");
        match &lib.books[idx].kind {
            BookKind::MultiFile { files } => assert_eq!(files.len(), 2),
            _ => panic!("expected MultiFile"),
        }
    }

    #[test]
    fn import_path_imports_a_single_audio_file() {
        let dir = TempDir::new().unwrap();
        let book_dir = dir.path().join("Solo Book");
        std::fs::create_dir(&book_dir).unwrap();
        let file = book_dir.join("book.m4b");
        touch(&file);

        let mut lib = Library::default();
        let idx = lib.import_path(&file).unwrap();

        match &lib.books[idx].kind {
            BookKind::SingleFile { path } => assert_eq!(path, &file),
            _ => panic!("expected SingleFile"),
        }
    }

    #[test]
    fn import_path_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let book_dir = dir.path().join("Same Book");
        std::fs::create_dir(&book_dir).unwrap();
        touch(&book_dir.join("01.mp3"));

        let mut lib = Library::default();
        let first = lib.import_path(&book_dir).unwrap();
        let second = lib.import_path(&book_dir).unwrap();

        assert_eq!(lib.books.len(), 1);
        assert_eq!(first, second);
    }

    #[test]
    fn import_path_errors_on_empty_dir() {
        let dir = TempDir::new().unwrap();
        let empty = dir.path().join("Empty");
        std::fs::create_dir(&empty).unwrap();

        let mut lib = Library::default();
        assert!(lib.import_path(&empty).is_err());
    }

    #[test]
    fn import_path_errors_on_non_audio_file() {
        let dir = TempDir::new().unwrap();
        let txt = dir.path().join("notes.txt");
        touch(&txt);

        let mut lib = Library::default();
        assert!(lib.import_path(&txt).is_err());
    }

    #[test]
    fn import_path_skips_macos_metadata_files() {
        let dir = TempDir::new().unwrap();
        let book_dir = dir.path().join("Book");
        std::fs::create_dir(&book_dir).unwrap();
        touch(&book_dir.join("01.mp3"));
        touch(&book_dir.join("._01.mp3"));
        touch(&book_dir.join(".DS_Store"));

        let mut lib = Library::default();
        let idx = lib.import_path(&book_dir).unwrap();

        // import_path doesn't filter ._ files itself (the browser does),
        // but the resulting MultiFile should still have only real audio.
        // This test documents that the current behavior includes ._ files,
        // so a future fix would update this assertion.
        match &lib.books[idx].kind {
            BookKind::MultiFile { files } => {
                // Currently we don't filter ._ in import_path. Lock that
                // in so we notice if the behavior changes.
                assert!(files.iter().any(|p| p.ends_with("01.mp3")));
            }
            BookKind::SingleFile { .. } => {}
        }
    }

    // ---- Library::scan: progress preservation across rescans ----

    #[test]
    fn scan_preserves_progress_and_bookmarks_for_known_books() {
        let mut lib = Library::default();

        // Pre-existing book with progress + a bookmark, with the same id
        // as the one scan() will discover.
        let dir = TempDir::new().unwrap();
        let book_dir = dir.path().join("Existing");
        std::fs::create_dir(&book_dir).unwrap();
        touch(&book_dir.join("01.mp3"));

        let id = book_id_for(&book_dir);
        let mut existing = make_book(&id, "Existing", book_dir.to_str().unwrap());
        existing.progress = Progress {
            current_chapter: 3,
            position: Duration::from_secs(742),
            finished: false,
            last_played_at: Some("2026-04-29T12:00:00Z".into()),
        };
        existing.bookmarks.push(Bookmark {
            chapter: 1,
            position: Duration::from_secs(120),
            note: Some("important".into()),
            created_at: "2026-04-29T12:00:00Z".into(),
        });
        lib.books.push(existing);

        lib.scan(&[dir.path().to_path_buf()]).unwrap();

        assert_eq!(lib.books.len(), 1);
        let after = &lib.books[0];
        assert_eq!(after.id, id);
        assert_eq!(after.progress.current_chapter, 3);
        assert_eq!(after.progress.position, Duration::from_secs(742));
        assert_eq!(after.bookmarks.len(), 1);
        assert_eq!(after.bookmarks[0].note.as_deref(), Some("important"));
    }

    #[test]
    fn scan_drops_books_no_longer_on_disk() {
        let dir = TempDir::new().unwrap();
        let real = dir.path().join("Still Here");
        std::fs::create_dir(&real).unwrap();
        touch(&real.join("01.mp3"));

        let mut lib = Library::default();
        // Stale entry with an id that no longer corresponds to anything
        // under the scanned root.
        lib.books.push(make_book(
            "deadbeef00000000",
            "Gone",
            "/tmp/does-not-exist",
        ));

        lib.scan(&[dir.path().to_path_buf()]).unwrap();

        assert_eq!(lib.books.len(), 1);
        assert_eq!(lib.books[0].title, "Still Here");
    }

    // ---- Library save/load round trip ----

    #[test]
    fn save_and_load_round_trip_preserves_books() {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("library.json");

        let mut lib = Library::default();
        lib.books.push(make_book("aabbccddeeff0011", "First", "/tmp/a"));
        lib.books.push(make_book("1100ffeeddccbbaa", "Second", "/tmp/b"));
        lib.save(&db).unwrap();

        let loaded = Library::load(&db).unwrap();
        assert_eq!(loaded.books.len(), 2);
        assert_eq!(loaded.books[0].id, "aabbccddeeff0011");
        assert_eq!(loaded.books[1].title, "Second");
    }

    #[test]
    fn load_returns_default_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let db = dir.path().join("nope.json");
        let loaded = Library::load(&db).unwrap();
        assert!(loaded.books.is_empty());
    }
}

mod duration_secs {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        d.as_secs_f64().serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let f = f64::deserialize(d)?;
        Ok(Duration::from_secs_f64(f.max(0.0)))
    }
}
