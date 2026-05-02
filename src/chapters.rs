use anyhow::Result;
use std::path::Path;
use std::time::Duration;

use crate::library::Chapter;

/// Read chapter list from a single .m4b's chpl atom (Nero-style chapter
/// list, the most common in audiobook m4b files). Returns an empty Vec
/// if the file has no chapter atom — caller can synthesize chapters
/// from the file itself.
pub fn read_m4b(path: &Path) -> Result<Vec<Chapter>> {
    let tag = match mp4ameta::Tag::read_from_path(path) {
        Ok(t) => t,
        Err(e) => return Err(anyhow::anyhow!("{e}")),
    };

    let raw = tag.chapters();
    if raw.is_empty() {
        return Ok(vec![]);
    }

    // mp4ameta gives us each chapter's absolute start; durations have to
    // be derived as the gap to the next start, with the last running to
    // end-of-file.
    let total = tag.duration();
    let mut chapters = Vec::with_capacity(raw.len());
    for (i, c) in raw.iter().enumerate() {
        let next_start = raw
            .get(i + 1)
            .map(|n| n.start)
            .unwrap_or(total);
        let duration = next_start.saturating_sub(c.start);
        chapters.push(Chapter {
            title: if c.title.is_empty() {
                format!("Chapter {}", i + 1)
            } else {
                c.title.clone()
            },
            file_index: 0, // single-file book
            start: c.start,
            duration,
        });
    }
    Ok(chapters)
}

/// Build per-file chapters for a multi-file book. Title is the filename
/// stem with common track-number prefixes stripped ("01 - foo" -> "foo").
pub fn synthesize_multi_file(file_durations: &[(String, Duration)]) -> Vec<Chapter> {
    file_durations
        .iter()
        .enumerate()
        .map(|(i, (filename, dur))| Chapter {
            title: clean_filename_title(filename),
            file_index: i,
            start: Duration::ZERO,
            duration: *dur,
        })
        .collect()
}

/// Strip leading track numbers and a separator from a filename stem.
/// "01 - The Way of Kings" → "The Way of Kings"
/// "Chapter 03.mp3" → "Chapter 03" (no change beyond extension)
fn clean_filename_title(name: &str) -> String {
    // Drop file extension first.
    let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);

    // Common patterns: "01 ", "01 - ", "01. ", "01_".
    let trimmed = stem.trim_start();
    let bytes = trimmed.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    // No leading digits → return stem as-is.
    if i == 0 {
        return stem.to_string();
    }
    // Skip a single separator: space, dash, dot, underscore, optionally
    // surrounded by spaces.
    let rest = &trimmed[i..];
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(['-', '.', '_']).unwrap_or(rest);
    let rest = rest.trim_start();

    if rest.is_empty() {
        // The filename was just digits — keep the stem.
        stem.to_string()
    } else {
        rest.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthesize_uses_filename_titles() {
        let files = vec![
            ("01 - Prologue.mp3".into(), Duration::from_secs(120)),
            ("02 - Chapter One.mp3".into(), Duration::from_secs(300)),
        ];
        let chapters = synthesize_multi_file(&files);
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].title, "Prologue");
        assert_eq!(chapters[1].title, "Chapter One");
        assert_eq!(chapters[0].file_index, 0);
        assert_eq!(chapters[1].file_index, 1);
        assert_eq!(chapters[1].duration, Duration::from_secs(300));
    }

    #[test]
    fn clean_filename_title_strips_track_number() {
        assert_eq!(clean_filename_title("01 - Foo.mp3"), "Foo");
        assert_eq!(clean_filename_title("12_Bar.flac"), "Bar");
        assert_eq!(clean_filename_title("003. Baz Quux.m4a"), "Baz Quux");
        assert_eq!(clean_filename_title("07 The Title.mp3"), "The Title");
    }

    #[test]
    fn clean_filename_title_leaves_non_numeric_alone() {
        assert_eq!(clean_filename_title("Chapter 03.mp3"), "Chapter 03");
        assert_eq!(clean_filename_title("Prologue.mp3"), "Prologue");
    }

    #[test]
    fn clean_filename_title_keeps_pure_digit_stems() {
        // "001.mp3" — stem is just digits, don't return empty.
        assert_eq!(clean_filename_title("001.mp3"), "001");
    }
}
