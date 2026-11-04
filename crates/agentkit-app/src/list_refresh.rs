//! Window-local refresh ownership, invalidated whenever the workspace binding changes.
#[derive(Default)]
pub(super) struct ListRefresh {
    generation: u64,
    pending: Option<u64>,
}
impl ListRefresh {
    pub fn pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn begin(&mut self) -> Option<u64> {
        if self.pending() {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        self.pending = Some(self.generation);
        Some(self.generation)
    }
    pub fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = None;
    }
    pub fn finish(&mut self, lease: u64) -> bool {
        if self.pending != Some(lease) {
            return false;
        }
        self.pending = None;
        true
    }
}
#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    #[::core::prelude::v1::test]
    fn old_workspace_completion_cannot_unlock_new_refresh_and_switch_back_still_refreshes() {
        let mut state = super::ListRefresh::default();
        let old = state.begin().expect("first refresh");
        state.invalidate();
        assert!(!state.pending());
        let target = state.begin().expect("target refresh");
        assert!(!state.finish(old));
        assert!(state.pending());
        assert!(state.finish(target));
        assert!(!state.pending());
        state.invalidate();
        let returned = state.begin().expect("switch-back refresh");
        assert!(!state.finish(old));
        assert!(state.finish(returned));
    }
}
