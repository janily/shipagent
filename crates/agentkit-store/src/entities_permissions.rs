//! Client-owned approval operation state and ephemeral conversation annotations.
use super::annotations::RoundAnchor;
use agentkit_protocol::generated as g;
/// Acknowledged permission decision, never a claim inferred from a click.
#[derive(Clone, Debug)]
pub struct PermissionReceipt {
    /// Canonical request metadata captured before RPC.
    pub request_id: String,
    /// Bounded tool display name without command/description payloads.
    pub tool_name: String,
    /// Allowed or rejected.
    pub allowed: bool,
    /// Session-wide grant was explicitly selected.
    pub always: bool,
    /// User message owning the active assistant round, if any.
    pub anchor: RoundAnchor,
}
/// A backend-confirmed mode transition for the conversation.
#[derive(Clone, Debug)]
pub struct ModeNotice {
    /// Monotonic backend mode version.
    pub version: Option<f64>,
    /// New permission mode.
    pub mode: g::PermissionMode,
    /// User message owning this round, if any.
    pub anchor: RoundAnchor,
}
/// Per-session operation lease; survives cached view switches.
#[derive(Default)]
pub struct ApprovalState {
    /// Currently outstanding request ID and its operation generation.
    pub pending: Option<(String, u64)>,
    /// Current request error, retained when switching views.
    pub error: Option<(String, String)>,
    /// Bounded, in-memory acknowledged decisions.
    pub receipts: Vec<PermissionReceipt>,
    /// Bounded backend-confirmed mode changes.
    pub modes: Vec<ModeNotice>,
    generation: u64,
}
impl ApprovalState {
    /// Acquire one operation; another view cannot approve this request concurrently.
    pub fn begin(&mut self, request: &str) -> Option<u64> {
        if self.pending.is_some() {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        self.pending = Some((request.into(), self.generation));
        self.error = None;
        Some(self.generation)
    }
    /// Release only the matching request generation.
    pub fn finish(&mut self, request: &str, generation: u64) -> bool {
        if self.pending.as_ref() != Some(&(request.into(), generation)) {
            return false;
        }
        self.pending = None;
        true
    }
    /// Record a semantic mode transition once, even when backend/runtime callbacks duplicate it.
    pub fn mode_changed(
        &mut self,
        version: Option<f64>,
        mode: g::PermissionMode,
        anchor: RoundAnchor,
    ) -> bool {
        if self.modes.last().is_some_and(|m| m.mode == mode)
            || version.is_some() && self.modes.iter().any(|m| m.version == version)
        {
            return false;
        }
        self.modes.push(ModeNotice {
            version,
            mode,
            anchor,
        });
        self.retain_recent();
        true
    }
    /// Keep annotation memory bounded without changing canonical backend history.
    pub fn retain_recent(&mut self) {
        if self.receipts.len() > 128 {
            self.receipts.drain(..self.receipts.len() - 128);
        }
        if self.modes.len() > 128 {
            self.modes.drain(..self.modes.len() - 128);
        }
    }
}
