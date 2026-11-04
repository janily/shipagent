use super::*;
use agentkit_store::entities::permissions::PermissionReceipt;
impl WorkbenchController {
    pub(super) fn respond_to_permission(
        &mut self,
        api: WorkbenchApi,
        intent: WorkbenchIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let WorkbenchIntent::Permission {
            session_id,
            request_id,
            allowed,
            always,
        } = intent
        else {
            return;
        };
        let Some(store) = self.stores.get(&session_id).cloned() else {
            return;
        };
        let Some(request) = store
            .read(cx)
            .model
            .permissions
            .first()
            .filter(|r| r.request_id == request_id && r.session_id == session_id)
            .cloned()
        else {
            return;
        };
        if always && (!allowed || request.can_always_allow != Some(true)) {
            return;
        }
        let anchor = store.read(cx).annotations.current();
        let tool_name: String = request.tool_name.chars().take(256).collect();
        let Some(lease) = store.update(cx, |s, cx| {
            let token = s.approval.begin(&request_id);
            cx.notify();
            token
        }) else {
            return;
        };
        let epoch = self.epoch;
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = api
                    .respond_permission(&session_id, &request_id, allowed, always)
                    .await;
                let _ = weak.update_in(cx, |this, _, cx| {
                    if this.epoch != epoch
                        || this
                            .stores
                            .get(&session_id)
                            .is_none_or(|s| s.entity_id() != store.entity_id())
                    {
                        return;
                    }
                    let accepted = result.is_ok();
                    store.update(cx, |s, cx| {
                        if !s.approval.finish(&request_id, lease) {
                            return;
                        }
                        match result {
                            Ok(()) => {
                                s.model.permission_responded(&request_id);
                                s.approval.error = None;
                                s.approval.receipts.push(PermissionReceipt {
                                    request_id: request_id.clone(),
                                    tool_name,
                                    allowed,
                                    always,
                                    anchor,
                                });
                                s.approval.retain_recent();
                                cx.emit(
                                agentkit_store::core::session::SessionStoreEvent::TurnsChanged {
                                    from_turn: 0,
                                },
                            );
                            }
                            Err(error) => {
                                s.approval.error = Some((request_id.clone(), error.to_string()))
                            }
                        }
                        cx.notify();
                    });
                    if accepted {
                        this.list.update(cx, |s, cx| {
                            s.model.permission_responded(&session_id, &request_id);
                            cx.notify();
                        });
                    }
                });
            }));
    }
}
