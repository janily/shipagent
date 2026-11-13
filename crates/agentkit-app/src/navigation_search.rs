use super::*;
use agentkit_store::core::views::Mutation;
impl WorkbenchController {
    pub(in crate::controller) fn invalidate_navigation_summaries(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.list_refresh.mark_dirty();
        self.list.update(cx, |s, cx| {
            s.model.refresh_required = true;
            cx.notify();
        });
        self.route(Vec::new(), window, cx);
    }
    pub(super) fn search_navigation(
        &mut self,
        request: u64,
        query: String,
        api: WorkbenchApi,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigation_state.search = None;
        if query.is_empty() {
            return;
        }
        let Some(workspace) = self.workspace.as_ref().map(|w| w.id.clone()) else {
            return;
        };
        let epoch = self.epoch;
        let transport = self.transport_epoch;
        self.navigation_state.search = Some(cx.spawn_in(window, async move |weak, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            let result = api
                .search_session_content(&workspace, &query, &uuid::Uuid::new_v4().to_string())
                .await
                .map_err(|e| e.to_string());
            let _ = weak.update_in(cx, |this, _, cx| {
                if this.settings_operation_current(epoch, transport, cx)
                    && this.workspace.as_ref().is_some_and(|w| w.id == workspace)
                {
                    this.view
                        .update(cx, |v, cx| v.search_results(request, &query, result, cx));
                }
            });
        }));
    }
    pub(in crate::controller) fn retain_search_jump(&mut self, intent: &WorkbenchIntent) {
        if self.navigation_state.jump.as_ref().is_some_and(
            |(id, _)| !matches!(intent,WorkbenchIntent::OpenSession(target) if target==id),
        ) {
            self.navigation_state.jump = None;
        }
    }
    pub(in crate::controller) fn finish_search_jump(&mut self, cx: &mut Context<Self>) {
        let Some((id, query)) = self.navigation_state.jump.as_ref() else {
            return;
        };
        if !self
            .state
            .as_ref()
            .is_some_and(|s| s.read(cx).model.state.session.id == *id)
        {
            return;
        }
        let (id, query) = (id.clone(), query.clone());
        self.navigation_state.jump = None;
        self.view.update(cx, |v, cx| {
            if !v.jump_to_match(&id, &query, cx) {
                v.show_error(agentkit_ui::t!("ak.nav.match_missing"), false, cx);
            }
        });
    }
    pub(super) fn mutate_session(
        &mut self,
        id: String,
        mutation: Mutation,
        api: WorkbenchApi,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let binding = (self.epoch, self.transport_epoch);
        if self.navigation_state.commands.get(&id) == Some(&binding)
            || !self.list.read(cx).model.sessions.iter().any(|s| {
                s.id == id
                    && s.hidden != Some(true)
                    && self
                        .workspace
                        .as_ref()
                        .is_some_and(|w| w.id == s.workspace_id)
            })
        {
            return;
        }
        self.navigation_state.commands.insert(id.clone(), binding);
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = match mutation {
                    Mutation::Flag | Mutation::Unflag => {
                        api.set_session_flag(&id, mutation == Mutation::Flag).await
                    }
                    Mutation::Archive | Mutation::Unarchive => {
                        api.set_session_archived(&id, mutation == Mutation::Archive)
                            .await
                    }
                    Mutation::MarkRead | Mutation::MarkUnread => {
                        api.set_session_read(&id, mutation == Mutation::MarkRead)
                            .await
                    }
                };
                let _ = weak.update_in(cx, |this, window, cx| {
                    if this.navigation_state.commands.get(&id) != Some(&binding) {
                        return;
                    }
                    this.navigation_state.commands.remove(&id);
                    if !this.settings_operation_current(binding.0, binding.1, cx) {
                        return;
                    }
                    match result {
                        Ok(()) => this.invalidate_navigation_summaries(window, cx),
                        Err(e) => this
                            .view
                            .update(cx, |v, cx| v.show_error(e.to_string(), false, cx)),
                    }
                });
            }));
    }
}
