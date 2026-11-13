//! The pinned list DTO lacks pending-review state. History projection runs off the
//! UI thread, two requests at a time. A 10s pending-only recheck handles commands
//! for which v0.2.6 broadcasts no event; selecting Plans also refreshes it.
use super::*;
use std::collections::{HashMap, HashSet};
#[derive(Default)]
pub(super) struct Projection {
    binding: Option<(u64, u64, String)>,
    task: Option<Task<()>>,
    timer: Option<Task<()>>,
    versions: HashMap<String, (i64, bool)>,
    candidates: HashSet<String>,
    pending_checks: HashSet<String>,
}
impl Projection {
    fn begin_requests<'a>(&mut self, ids: impl IntoIterator<Item = &'a str>) {
        // A reply only acknowledges invalidations that existed when its read began.
        for id in ids {
            self.pending_checks.remove(id);
        }
    }
    fn record(&mut self, id: &str, version: (i64, bool), eligible: bool) {
        self.versions.insert(id.into(), version);
        if eligible {
            self.candidates.insert(id.into());
        } else {
            self.candidates.remove(id);
        }
    }
}
async fn pending(api: &WorkbenchApi, id: &str, history: bool) -> Result<(bool, bool), String> {
    let eligible = if history {
        let session = api.session_generated(id).await.map_err(|e| e.to_string())?;
        agentkit_store::core::views::pending_review(&session)
    } else {
        true
    };
    if eligible {
        return api
            .pending_plan(id)
            .await
            .map(|p| (p.is_none(), true))
            .map_err(|e| e.to_string());
    }
    Ok((false, false))
}
impl WorkbenchController {
    pub(in crate::controller) fn retry_plan_views(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.list.update(cx, |s, cx| {
            s.plan_error = false;
            cx.notify();
        });
        let state = &mut self.navigation_state.projection;
        state
            .pending_checks
            .extend(state.candidates.iter().cloned());
        self.refresh_plan_views(window, cx);
    }
    pub(in crate::controller) fn invalidate_plan_review(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigation_state
            .projection
            .pending_checks
            .insert(id.into());
        self.refresh_plan_views(window, cx);
    }
    pub(in crate::controller) fn refresh_plan_views(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.workspace_switching || self.list_refresh.pending() {
            return;
        }
        let Some(workspace) = self.workspace.as_ref().map(|w| w.id.clone()) else {
            return;
        };
        let Some(api) = self.settings_api(cx) else {
            return;
        };
        let binding = (self.epoch, self.transport_epoch, workspace.clone());
        let state = &mut self.navigation_state.projection;
        if state.binding.as_ref() != Some(&binding) {
            *state = Projection {
                binding: Some(binding.clone()),
                ..Default::default()
            };
            self.list.update(cx, |s, cx| {
                s.plan_error = false;
                s.plan_loading = false;
                cx.notify();
            });
        }
        if state.task.is_some() || self.list.read(cx).plan_error {
            return;
        }
        let work: Vec<_> = self
            .list
            .read(cx)
            .model
            .sessions
            .iter()
            .filter(|s| {
                s.workspace_id == workspace
                    && s.hidden != Some(true)
                    && s.is_archived != Some(true)
                    && !s.is_processing
            })
            .filter_map(|s| {
                let version = (s.last_message_at, s.is_processing);
                let history = state.versions.get(&s.id) != Some(&version)
                    || (state.pending_checks.contains(&s.id) && !state.candidates.contains(&s.id));
                (history || state.pending_checks.contains(&s.id))
                    .then(|| (s.id.clone(), version, history))
            })
            .collect();
        if work.is_empty() {
            return;
        }
        state.begin_requests(work.iter().map(|w| w.0.as_str()));
        self.list.update(cx, |s, cx| {
            s.plan_loading = true;
            cx.notify();
        });
        let worker = cx.background_executor().spawn(async move {
            let mut out = Vec::new();
            for pair in work.chunks(2) {
                let a = &pair[0];
                if let Some(b) = pair.get(1) {
                    let (left, right) =
                        smol::future::zip(pending(&api, &a.0, a.2), pending(&api, &b.0, b.2)).await;
                    out.push((a.clone(), left));
                    out.push((b.clone(), right));
                } else {
                    out.push((a.clone(), pending(&api, &a.0, a.2).await));
                }
            }
            out
        });
        state.task = Some(cx.spawn_in(window, async move |weak, cx| {
            let result = worker.await;
            let _ = weak.update_in(cx, |this, window, cx| {
                if this.navigation_state.projection.binding.as_ref() != Some(&binding) {
                    return;
                }
                this.navigation_state.projection.task = None;
                if !this.settings_operation_current(binding.0, binding.1, cx) {
                    return;
                }
                this.list.update(cx, |s, cx| {
                    s.plan_loading = false;
                    for ((id, version, _), review) in result {
                        if !s.model.sessions.iter().any(|session| {
                            session.id == id
                                && (session.last_message_at, session.is_processing) == version
                        }) {
                            continue;
                        }
                        let state = &mut this.navigation_state.projection;
                        match review {
                            Ok((review, eligible)) => {
                                state.record(&id, version, eligible);
                                if review {
                                    s.model.plans.insert(id);
                                } else {
                                    s.model.plans.remove(&id);
                                }
                            }
                            Err(_) => s.plan_error = true,
                        }
                    }
                    cx.notify();
                });
                this.install_plan_recheck(window, cx);
                this.refresh_plan_views(window, cx);
            });
        }));
    }
    fn install_plan_recheck(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = &mut self.navigation_state.projection;
        if state.candidates.is_empty() || state.timer.as_ref().is_some_and(|t| !t.is_ready()) {
            return;
        }
        let binding = state.binding.clone();
        state.timer = Some(cx.spawn_in(window, async move |weak, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(10))
                    .await;
                let again = weak
                    .update_in(cx, |this, window, cx| {
                        let state = &mut this.navigation_state.projection;
                        if this.quitting.get()
                            || state.binding != binding
                            || state.candidates.is_empty()
                        {
                            return false;
                        }
                        state
                            .pending_checks
                            .extend(state.candidates.iter().cloned());
                        this.refresh_plan_views(window, cx);
                        true
                    })
                    .unwrap_or(false);
                if !again {
                    break;
                }
            }
        }));
    }
}

#[cfg(test)]
mod tests {
    #[::core::prelude::v1::test]
    fn projection_reply_cannot_consume_an_invalidation_that_arrived_in_flight() {
        let mut state = super::Projection::default();
        state.pending_checks.insert("s".into());
        state.begin_requests(["s"]);
        state.pending_checks.insert("s".into());
        state.record("s", (1, false), true);
        assert!(state.pending_checks.contains("s"));
    }
}
