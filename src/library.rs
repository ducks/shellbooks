use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

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

    /// Walk every configured library path and add new books, preserving
    /// existing progress + bookmarks for books we already knew about.
    pub fn scan(&mut self, _roots: &[PathBuf]) -> Result<()> {
        // TODO: walkdir each root, group sibling audio files into books,
        // call metadata::extract for tags, chapters::extract for chapter
        // info, merge with self.books matched by id (hash of root path).
        Ok(())
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
