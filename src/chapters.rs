use anyhow::Result;
use std::path::Path;
use std::time::Duration;

use crate::library::Chapter;

/// Read chapter list from a single .m4b's nero/quicktime atoms.
/// For multi-file books, callers synthesize one Chapter per file using the
/// file's filename as the title.
pub fn read_m4b(_path: &Path) -> Result<Vec<Chapter>> {
    // TODO: mp4ameta::Tag::read_from_path then walk chapters atom.
    Ok(vec![])
}

/// Build per-file chapters for a multi-file book, given each file's duration.
pub fn synthesize_multi_file(file_durations: &[(String, Duration)]) -> Vec<Chapter> {
    file_durations
        .iter()
        .enumerate()
        .map(|(i, (title, dur))| Chapter {
            title: title.clone(),
            file_index: i,
            start: Duration::ZERO,
            duration: *dur,
        })
        .collect()
}
