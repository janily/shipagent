use std::collections::{HashSet, VecDeque};
#[derive(Default)]
pub(super) struct ApprovalNotifications {
    seen: HashSet<(String, String)>,
    order: VecDeque<(String, String)>,
}
impl ApprovalNotifications {
    pub fn should_send(
        &mut self,
        session: &str,
        request: &str,
        active: bool,
        enabled: bool,
    ) -> bool {
        let key = (session.into(), request.into());
        if !self.seen.insert(key.clone()) {
            return false;
        }
        self.order.push_back(key);
        if self.order.len() > 512
            && let Some(key) = self.order.pop_front()
        {
            self.seen.remove(&key);
        }
        enabled && !active
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn background_only_and_replay_does_not_repeat_or_leak_between_sessions() {
        let mut n = ApprovalNotifications::default();
        assert!(!n.should_send("a", "r", true, true));
        assert!(!n.should_send("a", "r", false, true));
        assert!(n.should_send("b", "r", false, true));
        assert!(!n.should_send("b", "r", false, true));
        assert!(!n.should_send("c", "r", false, false));
    }
}
