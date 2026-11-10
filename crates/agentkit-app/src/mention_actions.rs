use super::*;
use agentkit_protocol::generated as g;
use agentkit_store::core::mentions;
impl WorkbenchController {
    fn mention_directory(&self, store: &Entity<SessionStore>, cx: &App) -> Option<String> {
        let workspace = self.workspace_store.read(cx);
        store
            .read(cx)
            .model
            .state
            .session
            .working_directory
            .clone()
            .or_else(|| {
                workspace
                    .settings
                    .as_ref()
                    .and_then(|s| s.working_directory.clone())
            })
            .or_else(|| workspace.workspace.as_ref().map(|w| w.root_path.clone()))
    }
    pub(super) fn load_mention_skills(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(store) = self
            .state
            .as_ref()
            .filter(|s| s.read(cx).model.state.session.id == id)
            .cloned()
        else {
            return;
        };
        let directory = self.mention_directory(&store, cx);
        if self.workspace_store.read(cx).skills_directory == directory && directory.is_some() {
            return;
        }
        let Some(api) = self.settings_api(cx) else {
            return;
        };
        self.mention_skill_request = self.mention_skill_request.wrapping_add(1);
        let request = self.mention_skill_request;
        let workspace = self.workspace_store.read(cx).id.clone();
        let epoch = self.epoch;
        let transport = self.transport_epoch;
        self.operations.push(cx.spawn(async move |weak, cx| {
            let result = api.session_skills(&workspace, directory.as_deref()).await;
            let _ = weak.update(cx, |this, cx| {
                if this.mention_skill_request != request
                    || !this.settings_operation_current(epoch, transport, cx)
                    || this
                        .state
                        .as_ref()
                        .is_none_or(|s| s.entity_id() != store.entity_id())
                    || this.mention_directory(&store, cx) != directory
                {
                    return;
                }
                if let Err(e) = &result {
                    this.view.update(cx, |v, cx| {
                        v.show_error(
                            agentkit_ui::t!("ak.mention.failed", error = e.to_string()),
                            false,
                            cx,
                        )
                    });
                }
                if let Ok(skills) = result {
                    this.workspace_store.update(cx, |s, cx| {
                        s.skills = skills;
                        s.skills_directory = directory;
                        cx.notify();
                    });
                }
            });
        }));
    }
    pub(super) fn mention_options(&self, text: &str, cx: &App) -> g::SendMessageOptions {
        let workspace = self.workspace_store.read(cx);
        let slug = workspace
            .workspace
            .as_ref()
            .map(|w| w.slug.as_str())
            .unwrap_or_default();
        let directory = self
            .state
            .as_ref()
            .and_then(|s| self.mention_directory(s, cx));
        let skills = mentions::catalog(
            &workspace.skills,
            slug,
            workspace.skills_directory.as_deref(),
            directory.as_deref(),
        );
        mentions::metadata(text, &skills)
    }
    pub(super) fn search_mentions(
        &mut self,
        id: String,
        request: u64,
        query: String,
        api: WorkbenchApi,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self
            .state
            .as_ref()
            .filter(|s| s.read(cx).model.state.session.id == id)
            .cloned()
        else {
            return;
        };
        let base = self.mention_directory(&store, cx);
        let epoch = self.epoch;
        let transport = self.transport_epoch;
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                let result = match &base {
                    Some(base) => api
                        .search_mentions(base, &query)
                        .await
                        .map_err(|e| e.to_string()),
                    None => Ok(Vec::new()),
                };
                let _ = weak.update_in(cx, |this, window, cx| {
                    if this.mention_directory(&store, cx) != base
                        || !this.settings_operation_current(epoch, transport, cx)
                        || this
                            .state
                            .as_ref()
                            .is_none_or(|s| s.entity_id() != store.entity_id())
                    {
                        return;
                    }
                    this.view.update(cx, |v, cx| {
                        v.mention_results(&id, request, &query, result, window, cx)
                    });
                });
            }));
    }
}

#[cfg(test)]
#[path = "mention_tests.rs"]
mod tests;
