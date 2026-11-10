//! Transient native plan presentation; durable execution remains backend-owned.
use std::collections::{HashMap, HashSet};
/// View cache and acknowledged acceptance state for one opened session.
#[derive(Default)]
pub struct PlanState {
    /// Loaded Markdown or a sanitized read failure, indexed by canonical message ID.
    pub content: HashMap<String, Result<String, ()>>,
    /// File reads currently in flight.
    pub loading: HashMap<String, (u64, u64)>,
    /// Locally acknowledged plan operations, before the next snapshot/event.
    pub accepted: HashSet<String>,
    /// An explicit plan action is in flight.
    pub pending: Option<u128>,
    /// Pending backend execution path; independent of permanent acceptance.
    pub pending_execution: Option<String>,
    /// Whether the pending record still requires a successful manual compaction.
    pub awaiting_compaction: bool,
}

impl PlanState {
    /// Reserve one file read per message and connection generation.
    pub fn begin_read(&mut self, id: &str, generation: (u64, u64)) -> bool {
        if self.content.get(id).is_some_and(Result::is_ok)
            || self.loading.get(id) == Some(&generation)
        {
            return false;
        }
        self.content.remove(id);
        self.loading.insert(id.into(), generation);
        true
    }
    /// Old callbacks cannot clear or replace a newer generation's read.
    pub fn finish_read(
        &mut self,
        id: &str,
        generation: (u64, u64),
        result: Result<String, ()>,
    ) -> bool {
        if self.loading.get(id) != Some(&generation) {
            return false;
        }
        self.loading.remove(id);
        self.content.insert(id.into(), result);
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delayed_old_read_cannot_unblock_or_overwrite_reconnected_read() {
        let mut p = PlanState::default();
        assert!(p.begin_read("p", (1, 1)));
        assert!(!p.begin_read("p", (1, 1)));
        assert!(p.begin_read("p", (2, 2)));
        assert!(!p.finish_read("p", (1, 1), Err(())));
        assert_eq!(p.loading.get("p"), Some(&(2, 2)));
        assert!(p.finish_read("p", (2, 2), Ok("new".into())));
        assert!(p.loading.is_empty());
        assert_eq!(
            p.content.get("p").and_then(|r| r.as_deref().ok()),
            Some("new")
        );
    }
}
