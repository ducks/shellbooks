use std::path::{Path, PathBuf};

use crate::list::ListSelector;

#[derive(Debug, Clone)]
pub enum BrowserItem {
    /// `..` — go up to parent directory.
    UpDirectory,
    /// A file or folder under the current directory.
    Entry(PathBuf),
}

pub struct BrowserState {
    pub current_dir: PathBuf,
    pub list: ListSelector<BrowserItem>,
}

impl BrowserState {
    pub fn new() -> Self {
        let current_dir = std::env::var("HOME")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let entries = read_dir_items(&current_dir);

        Self {
            current_dir,
            list: ListSelector::new(entries),
        }
    }

    /// Descend into the selected directory or surface it for import.
    /// Returns `Some(path)` when the selection is something the caller
    /// should consider importing — a directory containing audio files,
    /// or a single audio file. `None` means we either went up or
    /// descended into a directory of subdirectories.
    pub fn open_selected(&mut self) -> Option<PathBuf> {
        match self.list.selected_item().cloned() {
            Some(BrowserItem::UpDirectory) => {
                self.go_up();
                None
            }
            Some(BrowserItem::Entry(path)) if path.is_dir() => {
                if dir_contains_audio(&path) {
                    // Bottom-of-tree directory the user is asking us to
                    // treat as a book. Caller decides what to do with it.
                    return Some(path);
                }
                self.current_dir = path.clone();
                self.list.set_entries(read_dir_items(&self.current_dir));
                None
            }
            Some(BrowserItem::Entry(path)) if is_audio_file(&path) => Some(path),
            _ => None,
        }
    }

    pub fn go_up(&mut self) {
        if let Some(parent) = self.current_dir.parent() {
            self.current_dir = parent.to_path_buf();
            let entries = read_dir_items(&self.current_dir);
            self.list.set_entries(entries);
        }
    }

    pub fn move_up(&mut self) {
        self.list.move_up();
    }

    pub fn move_down(&mut self) {
        self.list.move_down();
    }

    pub fn go_to_top(&mut self) {
        self.list.go_to_top();
    }

    pub fn go_to_bottom(&mut self) {
        self.list.go_to_bottom();
    }
}

pub fn read_dir_items(dir: &Path) -> Vec<BrowserItem> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![BrowserItem::UpDirectory];
    };

    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            // Skip dotfiles to keep the listing focused.
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|s| !s.starts_with('.'))
                .unwrap_or(false)
        })
        .map(BrowserItem::Entry)
        .collect();

    paths.sort_by_key(|item| match item {
        BrowserItem::Entry(path) => path.clone(),
        _ => PathBuf::new(),
    });

    let mut all = vec![BrowserItem::UpDirectory];
    all.extend(paths);
    all
}

fn is_audio_file(path: &Path) -> bool {
    crate::library::is_audio_file(path)
}

/// Does this directory have at least one audio file directly inside it?
/// We don't recurse — the user explicitly picks the book directory.
fn dir_contains_audio(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries
        .filter_map(|e| e.ok())
        .any(|e| e.path().is_file() && is_audio_file(&e.path()))
}
