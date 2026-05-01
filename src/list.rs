use ratatui::widgets::ListState;

/// A scrollable selection over a Vec, mirroring shelltrax's ListSelector.
/// Owns both the raw entries and a ratatui ListState so renderers can
/// pass the latter directly to a List widget.
pub struct ListSelector<T> {
    pub entries: Vec<T>,
    pub selected: usize,
    pub state: ListState,
}

impl<T> ListSelector<T> {
    pub fn new(entries: Vec<T>) -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            entries,
            selected: 0,
            state,
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.state.select(Some(self.selected));
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
            self.state.select(Some(self.selected));
        }
    }

    pub fn set_entries(&mut self, entries: Vec<T>) {
        self.entries = entries;
        self.selected = 0;
        self.state.select(Some(0));
    }

    pub fn selected_item(&self) -> Option<&T> {
        self.entries.get(self.selected)
    }

    pub fn go_to_top(&mut self) {
        self.selected = 0;
        self.state.select(Some(0));
    }

    pub fn go_to_bottom(&mut self) {
        if !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
            self.state.select(Some(self.selected));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero() {
        let s: ListSelector<i32> = ListSelector::new(vec![10, 20, 30]);
        assert_eq!(s.selected, 0);
        assert_eq!(s.selected_item(), Some(&10));
    }

    #[test]
    fn move_down_advances_until_end() {
        let mut s = ListSelector::new(vec![1, 2, 3]);
        s.move_down();
        assert_eq!(s.selected, 1);
        s.move_down();
        s.move_down();
        s.move_down(); // past the end is a no-op
        assert_eq!(s.selected, 2);
        assert_eq!(s.selected_item(), Some(&3));
    }

    #[test]
    fn move_up_stops_at_zero() {
        let mut s = ListSelector::new(vec![1, 2, 3]);
        s.move_down();
        s.move_up();
        s.move_up();
        s.move_up();
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn go_to_top_and_bottom() {
        let mut s = ListSelector::new(vec!['a', 'b', 'c', 'd']);
        s.go_to_bottom();
        assert_eq!(s.selected_item(), Some(&'d'));
        s.go_to_top();
        assert_eq!(s.selected_item(), Some(&'a'));
    }

    #[test]
    fn set_entries_resets_selection() {
        let mut s = ListSelector::new(vec![1, 2, 3]);
        s.go_to_bottom();
        s.set_entries(vec![9, 8]);
        assert_eq!(s.selected, 0);
        assert_eq!(s.selected_item(), Some(&9));
    }

    #[test]
    fn empty_list_handles_navigation_safely() {
        let mut s: ListSelector<i32> = ListSelector::new(vec![]);
        s.move_down();
        s.move_up();
        s.go_to_bottom();
        s.go_to_top();
        assert_eq!(s.selected_item(), None);
    }
}
