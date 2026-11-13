//! Built-in navigation policy, based on mkagent v0.2.6 AppShell and spec04's six views.
use agentkit_protocol::generated as g;
use std::collections::HashSet;
/// Window-local built-in session filter. Flags never change recent-activity ordering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    /// Visible, non-archived sessions.
    #[default]
    All,
    /// Non-archived unread sessions.
    Unread,
    /// Non-archived flagged sessions.
    Flagged,
    /// Non-archived processing sessions.
    Running,
    /// Non-archived sessions awaiting plan review.
    Plans,
    /// Archived sessions only.
    Archived,
}
impl View {
    /// Stable UI/count ordering.
    pub const ALL: [Self; 6] = [
        Self::All,
        Self::Unread,
        Self::Flagged,
        Self::Running,
        Self::Plans,
        Self::Archived,
    ];
    /// Canonical membership, shared by title and body results and badges.
    pub fn includes(
        self,
        s: &g::Session,
        plans: &HashSet<String>,
        workspace: Option<&str>,
    ) -> bool {
        if s.hidden == Some(true) || workspace.is_some_and(|w| s.workspace_id != w) {
            return false;
        }
        if self == Self::Archived {
            return s.is_archived == Some(true);
        }
        if s.is_archived == Some(true) {
            return false;
        }
        match self {
            Self::All => true,
            Self::Unread => s.has_unread == Some(true),
            Self::Flagged => s.is_flagged == Some(true),
            Self::Running => s.is_processing,
            Self::Plans => !s.is_processing && plans.contains(&s.id),
            Self::Archived => false,
        }
    }
}
/// View counts ignore the search query.
pub fn counts(
    sessions: &[g::Session],
    plans: &HashSet<String>,
    workspace: Option<&str>,
) -> [usize; 6] {
    View::ALL.map(|v| {
        sessions
            .iter()
            .filter(|s| v.includes(s, plans, workspace))
            .count()
    })
}
/// Stable recent-first indices for immediate title filtering.
pub fn indices(
    sessions: &[g::Session],
    plans: &HashSet<String>,
    workspace: Option<&str>,
    view: View,
    query: &str,
) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let mut result: Vec<_> = sessions
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            view.includes(s, plans, workspace)
                && (query.is_empty()
                    || s.name
                        .as_deref()
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(&query))
        })
        .map(|(i, _)| i)
        .collect();
    result.sort_by(|a, b| {
        sessions[*b]
            .last_message_at
            .cmp(&sessions[*a].last_message_at)
            .then_with(|| sessions[*a].id.cmp(&sessions[*b].id))
    });
    result
}
/// An explicit desired state, captured from the canonical summary when a menu opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
pub enum Mutation {
    /// Set flag.
    Flag,
    /// Clear flag.
    Unflag,
    /// Archive.
    Archive,
    /// Restore from archive.
    Unarchive,
    /// Mark read.
    MarkRead,
    /// Mark unread.
    MarkUnread,
}

/// Last visible user/plan boundary, independent of later assistant commentary.
/// Live list badges use plan_submitted; cold starts reconstruct from canonical history.
pub fn pending_review(session: &g::Session) -> bool {
    session
        .messages
        .iter()
        .rev()
        .find(|m| {
            m.hidden != Some(true) && matches!(m.role, g::MessageRole::User | g::MessageRole::Plan)
        })
        .is_some_and(|m| m.role == g::MessageRole::Plan)
}
