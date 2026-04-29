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
