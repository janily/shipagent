use super::*;
use agentkit_protocol::generated as g;
use agentkit_store::core::plan_dispatch as policy;
#[path = "plan_worker.rs"]
mod worker;
impl WorkbenchController {
    pub(super) fn load_plan(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(store) = self.state.clone() else {
            return;
        };
        let path = store
            .read(cx)
            .model
            .state
            .session
            .messages
            .iter()
            .find(|m| m.id == id && m.role == g::MessageRole::Plan)
            .and_then(policy::path)
            .map(str::to_owned);
        let Some(path) = path else {
            return;
        };
        let epoch = self.epoch;
        let transport = self.transport_epoch;
        let generation = (epoch, transport);
        if !store.update(cx, |s, cx| {
            let start = s.plan.begin_read(&id, generation);
            cx.notify();
            start
        }) {
            return;
        }
        let Some(api) = self.settings_api(cx) else {
            store.update(cx, |s, cx| {
                s.plan.finish_read(&id, generation, Err(()));
                cx.notify();
            });
            return;
        };
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = api.plan_text(&path).await.map_err(|_| ());
                let _ = weak.update_in(cx, |this, _, cx| {
                    let result = if this.settings_operation_current(epoch, transport, cx) {
                        result
                    } else {
                        Err(())
                    };
                    store.update(cx, |s, cx| {
                        if s.plan.finish_read(&id, generation, result) {
                            cx.emit(
                                agentkit_store::core::session::SessionStoreEvent::TurnsChanged {
                                    from_turn: s.model.turn_index(&id).unwrap_or(0),
                                },
                            );
                            cx.notify();
                        }
                    });
                });
            }));
    }
    pub(super) fn accept_plan(
        &mut self,
        message: String,
        compact: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.state.clone() else {
            return;
        };
        if store.read(cx).model.busy()
            || store.read(cx).model.has_pending_send()
            || store.read(cx).attachments.sending
            || store.read(cx).selection_pending
            || store.read(cx).plan.pending.is_some()
        {
            return;
        }
        let Some(api) = self.settings_api(cx) else {
            return;
        };
        self.save_draft(cx);
        let model = &store.read(cx).model;
        if model
            .state
            .session
            .messages
            .iter()
            .rev()
            .find(|m| m.role == g::MessageRole::Plan && m.hidden != Some(true))
            .is_none_or(|m| m.id != message)
            || policy::accepted(&model.state.session, &message)
            || store.read(cx).plan.accepted.contains(&message)
        {
            return;
        }
        let id = model.state.session.id.clone();
        let draft = model.draft.text.clone();
        let revision = model.draft_revision();
        let Some(instance) = api.snapshot().client_id else {
            return;
        };
        let Some(mut flight) = cx
            .global::<SharedAppState>()
            .2
            .request(&format!("{instance}:{}:{id}", self.transport_epoch))
        else {
            self.view.update(cx, |v, cx| {
                v.show_error(agentkit_ui::t!("ak.plan.failed"), false, cx)
            });
            return;
        };
        let operation = uuid::Uuid::new_v4().as_u128();
        store.update(cx, |s, cx| {
            s.plan.pending = Some(operation);
            cx.notify();
        });
        let epoch = self.epoch;
        let transport = self.transport_epoch;
        let approved = agentkit_ui::t!("ak.plan.approved");
        let additional = agentkit_ui::t!("ak.plan.additional");
        let (tx, rx) = async_channel::bounded(1);
        let target = id.clone();
        let snapshot = draft.clone();
        smol::spawn(async move {
            let result = worker::accept(
                &api,
                &target,
                &message,
                compact,
                &snapshot,
                &approved,
                &additional,
            )
            .await
            .map(|_| message);
            let dispatch_failed = if flight.again() {
                worker::drain(&api, &target, &approved, &additional, flight)
                    .await
                    .is_err()
            } else {
                false
            };
            let _ = tx.send((result, dispatch_failed)).await;
        })
        .detach();
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let Ok((result, dispatch_failed)) = rx.recv().await else {
                    return;
                };
                let _ = weak.update_in(cx, |this, window, cx| {
                    store.update(cx, |s, cx| {
                        if s.plan.pending == Some(operation) {
                            s.plan.pending = None;
                            cx.notify();
                        }
                    });
                    if !this.settings_operation_current(epoch, transport, cx) {
                        return;
                    }
                    let clear = store.update(cx, |s, cx| {
                        if let Ok(id) = &result {
                            if compact {
                                s.plan.awaiting_compaction = true;
                                s.plan.pending_execution = s
                                    .model
                                    .state
                                    .session
                                    .messages
                                    .iter()
                                    .find(|m| &m.id == id)
                                    .and_then(policy::path)
                                    .map(str::to_owned);
                            } else {
                                s.plan.accepted.insert(id.clone());
                            }
                        }
                        let clear = result.is_ok()
                            && s.model.draft_revision() == revision
                            && s.model.draft.text == draft;
                        if clear {
                            s.model.set_draft(String::new());
                        }
                        cx.emit(
                            agentkit_store::core::session::SessionStoreEvent::TurnsChanged {
                                from_turn: 0,
                            },
                        );
                        cx.notify();
                        clear
                    });
                    if this
                        .state
                        .as_ref()
                        .is_some_and(|s| s.entity_id() == store.entity_id())
                    {
                        this.view.update(cx, |v, cx| {
                            v.submission_finished(&draft, clear, window, cx);
                            if result.is_err() || dispatch_failed {
                                v.show_error(agentkit_ui::t!("ak.plan.failed"), false, cx);
                            }
                        });
                    }
                    this.wake_plan(&id, window, cx);
                });
            }));
    }
    pub(super) fn wake_plan(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(api) = self.settings_api(cx) else {
            return;
        };
        let Some(instance) = api.snapshot().client_id else {
            return;
        };
        let Some(flight) = cx
            .global::<SharedAppState>()
            .2
            .request(&format!("{instance}:{}:{id}", self.transport_epoch))
        else {
            return;
        };
        let Some(store) = self.stores.get(id).cloned() else {
            return;
        };
        let target = id.to_string();
        let epoch = self.epoch;
        let transport = self.transport_epoch;
        let approved = agentkit_ui::t!("ak.plan.approved");
        let additional = agentkit_ui::t!("ak.plan.additional");
        let (tx, rx) = async_channel::bounded(1);
        smol::spawn(async move {
            let result = worker::drain(&api, &target, &approved, &additional, flight).await;
            let _ = tx.send(result).await;
        })
        .detach();
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let Ok(result) = rx.recv().await else {
                    return;
                };
                let _ = weak.update_in(cx, |this, _, cx| {
                    if !this.settings_operation_current(epoch, transport, cx) {
                        return;
                    }
                    match result {
                        Ok(outcome) => {
                            store.update(cx, |s, cx| {
                                match outcome {
                                    worker::Outcome::Unchanged => {}
                                    worker::Outcome::None => {
                                        s.plan.pending_execution = None;
                                    }
                                    worker::Outcome::Pending {
                                        path,
                                        awaiting_compaction,
                                    } => {
                                        s.plan.pending_execution = Some(path);
                                        s.plan.awaiting_compaction = awaiting_compaction;
                                    }
                                    worker::Outcome::Sent { path } => {
                                        s.plan.pending_execution = None;
                                        for m in &s.model.state.session.messages {
                                            if m.role == g::MessageRole::Plan
                                                && policy::path(m) == Some(path.as_str())
                                            {
                                                s.plan.accepted.insert(m.id.clone());
                                            }
                                        }
                                    }
                                }
                                cx.emit(
                                agentkit_store::core::session::SessionStoreEvent::TurnsChanged {
                                    from_turn: 0,
                                },
                            );
                                cx.notify();
                            });
                        }
                        Err(_) => {
                            if this
                                .state
                                .as_ref()
                                .is_some_and(|s| s.entity_id() == store.entity_id())
                            {
                                this.view.update(cx, |v, cx| {
                                    v.show_error(agentkit_ui::t!("ak.plan.failed"), false, cx)
                                });
                            }
                        }
                    }
                });
            }));
    }
}
