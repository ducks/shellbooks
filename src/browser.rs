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

    /// Descend into the selected directory, or — for an audio file —
    /// surface its path so the caller can play it inline. This never
    /// imports; importing is a separate explicit action via the `a`
    /// keybind, mirroring shelltrax.
    pub fn open_selected(&mut self) -> Option<PathBuf> {
        match self.list.selected_item().cloned() {
            Some(BrowserItem::UpDirectory) => {
                self.go_up();
                None
            }
            Some(BrowserItem::Entry(path)) if path.is_dir() => {
                self.current_dir = path.clone();
                self.list.set_entries(read_dir_items(&self.current_dir));
                None
            }
            Some(BrowserItem::Entry(path)) if is_audio_file(&path) => Some(path),
            _ => None,
        }
    }

    /// Returns the currently-selected path (without descending). Used
    /// by the `a` import action.
    pub fn selected_path(&self) -> Option<PathBuf> {
        match self.list.selected_item() {
            Some(BrowserItem::Entry(p)) => Some(p.clone()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn touch(p: &Path) {
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(p, b"").unwrap();
    }

    #[test]
    fn read_dir_items_returns_only_up_when_dir_unreadable() {
        let items = read_dir_items(Path::new("/this/path/does/not/exist"));
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], BrowserItem::UpDirectory));
    }

    #[test]
    fn read_dir_items_filters_dotfiles() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("visible.txt"));
        touch(&dir.path().join(".hidden"));

        let items = read_dir_items(dir.path());
        let names: Vec<String> = items
            .iter()
            .filter_map(|i| match i {
                BrowserItem::Entry(p) => Some(
                    p.file_name().unwrap().to_string_lossy().to_string(),
                ),
                _ => None,
            })
            .collect();
        assert_eq!(names, vec!["visible.txt"]);
    }

    #[test]
    fn dir_contains_audio_detects_one_audio_file() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("readme.txt"));
        assert!(!dir_contains_audio(dir.path()));
        touch(&dir.path().join("01.mp3"));
        assert!(dir_contains_audio(dir.path()));
    }

    #[test]
    fn dir_contains_audio_does_not_recurse() {
        let dir = TempDir::new().unwrap();
        let inner = dir.path().join("inner");
        std::fs::create_dir(&inner).unwrap();
        touch(&inner.join("01.mp3"));
        // No audio at the top level itself.
        assert!(!dir_contains_audio(dir.path()));
    }

    #[test]
    fn open_selected_descends_into_directory_of_directories() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();

        let mut state = BrowserState {
            current_dir: dir.path().to_path_buf(),
            list: ListSelector::new(read_dir_items(dir.path())),
        };
        // Move past UpDirectory to land on `sub`.
        state.move_down();
        assert!(state.open_selected().is_none());
        assert_eq!(state.current_dir, sub);
    }

    #[test]
    fn open_selected_descends_into_directory_with_audio() {
        // Per shelltrax convention, Enter only descends — it never
        // imports. Importing is a separate `a` action via selected_path().
        let dir = TempDir::new().unwrap();
        let book = dir.path().join("Book");
        std::fs::create_dir(&book).unwrap();
        touch(&book.join("01.mp3"));

        let mut state = BrowserState {
            current_dir: dir.path().to_path_buf(),
            list: ListSelector::new(read_dir_items(dir.path())),
        };
        state.move_down();
        let result = state.open_selected();
        assert!(result.is_none(), "open_selected should not return a path for a directory");
        assert_eq!(state.current_dir, book);
    }

    #[test]
    fn selected_path_returns_highlighted_directory() {
        let dir = TempDir::new().unwrap();
        let book = dir.path().join("Book");
        std::fs::create_dir(&book).unwrap();

        let mut state = BrowserState {
            current_dir: dir.path().to_path_buf(),
            list: ListSelector::new(read_dir_items(dir.path())),
        };
        state.move_down();
        assert_eq!(state.selected_path().as_deref(), Some(book.as_path()));
    }

    #[test]
    fn selected_path_returns_none_on_up_directory() {
        let dir = TempDir::new().unwrap();
        let state = BrowserState {
            current_dir: dir.path().to_path_buf(),
            list: ListSelector::new(read_dir_items(dir.path())),
        };
        // First entry is UpDirectory.
        assert!(state.selected_path().is_none());
    }

    #[test]
    fn open_selected_returns_path_for_single_audio_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("solo.m4b");
        touch(&file);

        let mut state = BrowserState {
            current_dir: dir.path().to_path_buf(),
            list: ListSelector::new(read_dir_items(dir.path())),
        };
        state.move_down();
        let result = state.open_selected();
        assert_eq!(result.as_deref(), Some(file.as_path()));
    }

    #[test]
    fn go_up_moves_to_parent() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();

        let mut state = BrowserState {
            current_dir: sub.clone(),
            list: ListSelector::new(read_dir_items(&sub)),
        };
        state.go_up();
        assert_eq!(state.current_dir, dir.path());
    }
}
