//! Window-local history, committed only after a session successfully opens.
#[derive(Default)]
pub(crate) struct History {
    workspace: String,
    entries: Vec<String>,
    cursor: usize,
    target: Option<usize>,
}
impl History {
    pub fn record(&mut self, workspace: &str, id: &str) {
        if self.workspace != workspace {
            self.workspace = workspace.into();
            self.entries.clear();
            self.target = None;
        }
        if let Some(index) = self.target.take()
            && self.entries.get(index).is_some_and(|s| s == id)
        {
            self.cursor = index;
            return;
        }
        if self.entries.get(self.cursor).is_some_and(|s| s == id) {
            return;
        }
        self.entries.truncate(self.cursor + 1);
        self.entries.push(id.into());
        self.cursor = self.entries.len() - 1;
    }
    pub fn step(&mut self, delta: isize, exists: impl Fn(&str) -> bool) -> Option<String> {
        let mut i = self.cursor as isize + delta;
        while i >= 0 && (i as usize) < self.entries.len() {
            if exists(&self.entries[i as usize]) {
                self.target = Some(i as usize);
                return Some(self.entries[i as usize].clone());
            }
            i += delta;
        }
        None
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn failed_navigation_does_not_move_and_new_navigation_forks_history() {
        let mut h = super::History::default();
        for id in ["a", "b", "c"] {
            h.record("w", id);
        }
        assert_eq!(h.step(-1, |_| true).as_deref(), Some("b"));
        assert_eq!(h.step(-1, |_| true).as_deref(), Some("b"));
        h.record("w", "b");
        h.record("w", "d");
        assert_eq!(h.step(1, |_| true), None);
        assert_eq!(h.step(-1, |s| s != "b").as_deref(), Some("a"));
        h.record("other", "x");
        assert_eq!(h.step(-1, |_| true), None);
    }
}
