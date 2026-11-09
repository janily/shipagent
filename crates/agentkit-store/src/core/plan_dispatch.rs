//! Persisted plan readiness, approval text and shared per-session wakeup ownership.
use agentkit_protocol::generated as g;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
/// Build exactly the upstream path-agnostic approval text using the current locale.
pub fn approval_message(approved: &str, additional: &str, draft: &str) -> String {
    let draft = draft.trim();
    if draft.is_empty() {
        approved.into()
    } else {
        format!("{approved}\n\n---\n\n**{additional}**\n\n{draft}")
    }
}
/// Notifications are wakeups only; persisted readiness and host claim authorize a send.
pub fn ready(pending: &g::PendingPlanExecution, busy: bool) -> bool {
    !busy && !pending.awaiting_compaction && pending.execution_dispatched != Some(true)
}
/// Canonical path, including v0.2.6's path-only plan message content.
pub fn path(message: &g::Message) -> Option<&str> {
    message
        .plan_path
        .as_deref()
        .filter(|p| !p.is_empty())
        .or_else(|| {
            let s = message.content.trim();
            (!s.contains('\n')
                && (s.starts_with('/')
                    || s.starts_with("\\\\")
                    || (s.as_bytes().get(1) == Some(&b':')
                        && s.as_bytes()
                            .get(2)
                            .is_some_and(|c| *c == b'\\' || *c == b'/'))))
            .then_some(s)
        })
}
/// Whether persisted messages already contain the approval following this plan.
pub fn accepted(session: &g::Session, id: &str) -> bool {
    let Some(index) = session
        .messages
        .iter()
        .position(|m| m.id == id && m.role == g::MessageRole::Plan)
    else {
        return false;
    };
    session.messages[index + 1..].iter().any(|m| {
        m.role == g::MessageRole::User
            && (m.content.starts_with("Plan approved, please execute.")
                || m.content.starts_with("计划已批准，请执行。"))
    })
}
/// Process-owned coordinator shared by existing application windows, with no static state.
#[derive(Clone, Default)]
pub struct Scheduler(Arc<Mutex<HashMap<String, bool>>>);
impl Scheduler {
    /// Start one worker, or coalesce another wakeup into the existing worker.
    pub fn request(&self, id: &str) -> Option<Flight> {
        let mut states = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(requested) = states.get_mut(id) {
            *requested = true;
            None
        } else {
            states.insert(id.into(), false);
            Some(Flight {
                scheduler: self.clone(),
                id: id.into(),
                owned: true,
            })
        }
    }
}
/// RAII single-flight lease; the worker owns it even after a view disappears.
pub struct Flight {
    scheduler: Scheduler,
    id: String,
    owned: bool,
}
impl Flight {
    /// Atomically drain a coalesced wakeup or release ownership before another request.
    pub fn again(&mut self) -> bool {
        let mut states = self.scheduler.0.lock().unwrap_or_else(|e| e.into_inner());
        if states.get(&self.id) == Some(&true) {
            states.insert(self.id.clone(), false);
            true
        } else {
            states.remove(&self.id);
            self.owned = false;
            false
        }
    }
}
impl Drop for Flight {
    fn drop(&mut self) {
        if self.owned {
            self.scheduler
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&self.id);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persisted_readiness_survives_restart_but_claimed_records_never_resend() {
        let mut p = g::PendingPlanExecution {
            plan_path: "/plan.md".into(),
            draft_input_snapshot: Some("snapshot".into()),
            awaiting_compaction: true,
            execution_dispatched: None,
        };
        assert!(!ready(&p, false));
        p.awaiting_compaction = false;
        assert!(ready(&p, false));
        assert!(!ready(&p, true));
        p.execution_dispatched = Some(true);
        assert!(!ready(&p, false));
    }
    #[test]
    fn wakeups_are_coalesced_and_released_atomically() {
        let s = Scheduler::default();
        let Some(mut f) = s.request("s") else {
            panic!()
        };
        assert!(s.request("s").is_none());
        assert!(f.again());
        assert!(!f.again());
        let Some(next) = s.request("s") else { panic!() };
        drop(f);
        assert!(s.request("s").is_none());
        drop(next);
        assert!(s.request("s").is_some());
        assert!(s.request("other").is_some());
    }
    #[test]
    fn approval_preserves_draft_and_has_no_plan_path() {
        assert_eq!(
            approval_message("approved", "additional", "  user\nnotes  "),
            "approved\n\n---\n\n**additional**\n\nuser\nnotes"
        );
        assert_eq!(
            approval_message("approved", "additional", " \n "),
            "approved"
        );
    }
}
