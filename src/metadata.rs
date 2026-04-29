use anyhow::Result;
use std::path::{Path, PathBuf};

/// Tags read out of an audio file. Sources: id3 for mp3, mp4ameta for m4b/m4a.
#[derive(Debug, Default, Clone)]
pub struct Tags {
    pub title: Option<String>,
    pub author: Option<String>,
    pub narrator: Option<String>,
    pub series: Option<String>,
    pub series_index: Option<u32>,
    pub year: Option<i32>,
    /// Cover art bytes (jpeg/png) embedded in tags, if any.
    pub embedded_cover: Option<Vec<u8>>,
}

pub fn read(_path: &Path) -> Result<Tags> {
    // TODO: dispatch on extension. id3::Tag::read_from_path for mp3,
    // mp4ameta::Tag::read_from_path for m4b/m4a. Map narrator from
    // "TCOM" or "©nrt" depending on source.
    Ok(Tags::default())
}

/// Look for `cover.jpg` / `cover.png` / `folder.jpg` next to the book.
pub fn sidecar_cover(book_root: &Path) -> Option<PathBuf> {
    for name in ["cover.jpg", "cover.jpeg", "cover.png", "folder.jpg"] {
        let candidate = book_root.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
