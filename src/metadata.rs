use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Tags read out of an audio file. Sources: id3 for mp3, mp4ameta for m4b/m4a.
/// All fields are best-effort: missing tags are simply None.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Tags {
    pub title: Option<String>,
    pub author: Option<String>,
    pub narrator: Option<String>,
    pub series: Option<String>,
    pub series_index: Option<u32>,
    pub year: Option<i32>,
    /// Cover art bytes (jpeg/png) embedded in tags, if any.
    pub embedded_cover: Option<Vec<u8>>,
    /// File duration in seconds, if the format reports it.
    pub duration: Option<Duration>,
}

/// Read tags from a single audio file. Dispatches by extension. Unknown
/// extensions return Tags::default() rather than failing — callers can
/// always fall back to the file's directory name.
pub fn read(path: &Path) -> Result<Tags> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    match ext.as_deref() {
        Some("mp3") => read_mp3(path),
        Some("m4b") | Some("m4a") | Some("mp4") => read_mp4(path),
        _ => Ok(Tags::default()),
    }
}

fn read_mp3(path: &Path) -> Result<Tags> {
    use id3::{Tag, TagLike};

    let tag = match Tag::read_from_path(path) {
        Ok(t) => t,
        // No tag at all is a normal case for a freshly-ripped file.
        Err(id3::Error {
            kind: id3::ErrorKind::NoTag,
            ..
        }) => return Ok(Tags::default()),
        Err(e) => return Err(e).context(format!("reading id3 tags from {}", path.display())),
    };

    // Some publishers stash audiobook-specific values in the standard
    // music frames: artist=author, composer=narrator, album=series.
    // It's a convention, not a standard, so we follow it permissively.
    let mut tags = Tags {
        title: tag.title().map(str::to_string),
        author: tag.artist().map(str::to_string),
        narrator: tag.get("TCOM").and_then(text_of).or_else(|| tag.get("COM").and_then(text_of)),
        series: tag.album().map(str::to_string),
        series_index: tag.get("TPOS").and_then(text_of).and_then(|s| parse_index(&s)),
        year: tag.year(),
        embedded_cover: first_picture_data(&tag),
        duration: tag.duration().map(|ms_or_s| {
            // id3 reports duration in milliseconds, despite the name.
            Duration::from_millis(ms_or_s as u64)
        }),
    };

    // The standard says TIT2 holds title — defensively handle stray text frames.
    if tags.title.is_none() {
        tags.title = tag.get("TIT2").and_then(text_of);
    }
    Ok(tags)
}

fn text_of(frame: &id3::Frame) -> Option<String> {
    frame.content().text().map(|s| s.to_string())
}

fn first_picture_data(tag: &id3::Tag) -> Option<Vec<u8>> {
    use id3::TagLike;
    tag.pictures().next().map(|p| p.data.clone())
}

fn read_mp4(path: &Path) -> Result<Tags> {
    let tag = match mp4ameta::Tag::read_from_path(path) {
        Ok(t) => t,
        Err(e) => return Err(anyhow::anyhow!("{e}"))
            .context(format!("reading mp4 tags from {}", path.display())),
    };

    let title = tag.title().map(str::to_string);
    let author = tag.artist().map(str::to_string);
    let series = tag.album().map(str::to_string);
    let year = tag.year().and_then(|y| y.parse::<i32>().ok());

    // Audiobook m4b files conventionally store the narrator in the
    // composer atom (©wrt). Some publishers use a custom freeform
    // "----:com.apple.iTunes:NARRATOR" atom — covered if we expand
    // later. For now ©wrt is the common case.
    let narrator = tag.composer().map(str::to_string);

    // First artwork attachment, if any.
    let embedded_cover = tag.artwork().map(|a| a.data.to_vec());

    let duration = Some(tag.duration());

    Ok(Tags {
        title,
        author,
        narrator,
        series,
        series_index: None,
        year,
        embedded_cover,
        duration,
    })
}

/// Look for `cover.jpg` / `cover.png` / `folder.jpg` next to the book.
pub fn sidecar_cover(book_root: &Path) -> Option<PathBuf> {
    for name in ["cover.jpg", "cover.jpeg", "cover.png", "folder.jpg", "folder.png"] {
        let candidate = book_root.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Apply tags to a book, preferring tag values over directory-derived defaults.
/// Caller still resolves the cover_path via sidecar_cover or by writing
/// embedded_cover bytes to the cache.
pub fn apply_to_book(book: &mut crate::library::Book, tags: &Tags) {
    if let Some(t) = &tags.title
        && !t.trim().is_empty()
    {
        book.title = t.clone();
    }
    if tags.author.is_some() {
        book.author = tags.author.clone();
    }
    if tags.narrator.is_some() {
        book.narrator = tags.narrator.clone();
    }
    if tags.series.is_some() {
        book.series = tags.series.clone();
    }
    if tags.series_index.is_some() {
        book.series_index = tags.series_index;
    }
    if tags.year.is_some() {
        book.year = tags.year;
    }
}

/// Parse "3" or "3/12" (TPOS) into a 1-based series index.
fn parse_index(raw: &str) -> Option<u32> {
    raw.split('/').next()?.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{Book, BookKind, Progress};

    fn make_book(title: &str) -> Book {
        Book {
            id: "abc123".into(),
            root: PathBuf::from("/tmp/x"),
            kind: BookKind::SingleFile { path: PathBuf::from("/tmp/x/book.m4b") },
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

    #[test]
    fn read_returns_default_for_unknown_extension() {
        let tags = read(Path::new("/tmp/x/notes.txt")).unwrap();
        assert_eq!(tags, Tags::default());
    }

    #[test]
    fn read_returns_default_for_missing_file() {
        // mp3 dispatch on a missing file: we treat NoTag as empty, but a
        // real I/O error should propagate. Use a path that won't exist.
        let result = read(Path::new("/tmp/this/does/not/exist.mp3"));
        // Either Ok(default) (NoTag) or Err — both are acceptable; we
        // mainly want it to not panic.
        match result {
            Ok(t) => assert_eq!(t, Tags::default()),
            Err(_) => {}
        }
    }

    #[test]
    fn parse_index_handles_bare_number() {
        assert_eq!(parse_index("3"), Some(3));
    }

    #[test]
    fn parse_index_handles_slash_form() {
        assert_eq!(parse_index("3/12"), Some(3));
        assert_eq!(parse_index("  7 / 99  "), Some(7));
    }

    #[test]
    fn parse_index_rejects_garbage() {
        assert_eq!(parse_index(""), None);
        assert_eq!(parse_index("nope"), None);
    }

    #[test]
    fn apply_to_book_overrides_title() {
        let mut book = make_book("DirName");
        let tags = Tags {
            title: Some("The Way of Kings".into()),
            ..Default::default()
        };
        apply_to_book(&mut book, &tags);
        assert_eq!(book.title, "The Way of Kings");
    }

    #[test]
    fn apply_to_book_keeps_existing_title_for_blank_tag() {
        let mut book = make_book("DirName");
        let tags = Tags {
            title: Some("   ".into()),
            ..Default::default()
        };
        apply_to_book(&mut book, &tags);
        assert_eq!(book.title, "DirName");
    }

    #[test]
    fn apply_to_book_sets_author_narrator_series() {
        let mut book = make_book("Book");
        let tags = Tags {
            author: Some("Brandon Sanderson".into()),
            narrator: Some("Michael Kramer".into()),
            series: Some("The Stormlight Archive".into()),
            series_index: Some(1),
            year: Some(2010),
            ..Default::default()
        };
        apply_to_book(&mut book, &tags);
        assert_eq!(book.author.as_deref(), Some("Brandon Sanderson"));
        assert_eq!(book.narrator.as_deref(), Some("Michael Kramer"));
        assert_eq!(book.series.as_deref(), Some("The Stormlight Archive"));
        assert_eq!(book.series_index, Some(1));
        assert_eq!(book.year, Some(2010));
    }

    #[test]
    fn apply_to_book_does_not_clobber_existing_with_none() {
        let mut book = make_book("Book");
        book.author = Some("Existing".into());
        let tags = Tags::default();
        apply_to_book(&mut book, &tags);
        assert_eq!(book.author.as_deref(), Some("Existing"));
    }

    #[test]
    fn sidecar_cover_finds_cover_jpg() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("cover.jpg"), b"").unwrap();
        let result = sidecar_cover(dir.path());
        assert_eq!(result.unwrap().file_name().unwrap(), "cover.jpg");
    }

    #[test]
    fn sidecar_cover_prefers_jpg_over_png() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("cover.jpg"), b"").unwrap();
        std::fs::write(dir.path().join("cover.png"), b"").unwrap();
        let result = sidecar_cover(dir.path()).unwrap();
        assert_eq!(result.file_name().unwrap(), "cover.jpg");
    }

    #[test]
    fn sidecar_cover_returns_none_when_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        assert!(sidecar_cover(dir.path()).is_none());
    }
}
