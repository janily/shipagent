//! Native annotation placement, indexed only when the canonical turn structure changes.
use crate::core::turns::Turn;
use std::collections::HashMap;
/// Stable user identity plus assistant segment separated by system-message boundaries.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RoundAnchor {
    /// Canonical owning user message, absent for an initial system round.
    pub user: Option<String>,
    /// Assistant segment within that user round.
    pub segment: usize,
}
/// Native row lookup for bounded client annotations; pure streaming deltas do not rebuild it.
#[derive(Default)]
pub struct AnnotationIndex {
    rows: HashMap<RoundAnchor, usize>,
    current: RoundAnchor,
}
impl AnnotationIndex {
    pub(crate) fn rebuild(&mut self, turns: &[Turn]) {
        self.rows.clear();
        let mut user = None;
        let mut segment = 0;
        self.current = RoundAnchor::default();
        for (row, turn) in turns.iter().enumerate() {
            match turn {
                Turn::User(t) if t.message.is_queued != Some(true) => {
                    user = Some(t.message.id.clone());
                    segment = 0;
                    self.current = RoundAnchor {
                        user: user.clone(),
                        segment,
                    };
                }
                Turn::Assistant(_) => {
                    let anchor = RoundAnchor {
                        user: user.clone(),
                        segment,
                    };
                    self.rows.insert(anchor.clone(), row);
                    self.current = anchor;
                    segment += 1;
                }
                Turn::System(_) => {
                    self.current = RoundAnchor {
                        user: user.clone(),
                        segment,
                    }
                }
                _ => {}
            }
        }
    }
    /// The current assistant segment, including an upcoming segment with no activity yet.
    pub fn current(&self) -> RoundAnchor {
        self.current.clone()
    }
    /// Exact currently materialized row, if that assistant segment exists.
    pub fn row(&self, anchor: &RoundAnchor) -> Option<usize> {
        self.rows.get(anchor).copied()
    }
}
