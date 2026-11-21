use super::*;
impl SessionSidebar {
    pub(super) fn query_changed(&mut self, cx: &mut Context<Self>) {
        let had_query = self.searching || self.search_error || !self.search_results.is_empty();
        self.search_request = self.search_request.wrapping_add(1);
        self.search_results.clear();
        self.search_error = false;
        let query = self.search.read(cx).value().trim().to_string();
        self.searching = self.ready && !query.is_empty();
        self.rebuild(cx);
        if !query.is_empty() || had_query {
            cx.emit(SidebarIntent::Search {
                request: self.search_request,
                query,
            });
        }
    }
    /// Apply only the currently visible, connected query's completion.
    pub fn search_results(
        &mut self,
        request: u64,
        query: &str,
        result: Result<Vec<g::SessionSearchResult>, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.ready
            || request != self.search_request
            || self.search.read(cx).value().trim() != query
        {
            return;
        }
        self.searching = false;
        self.search_error = result.is_err();
        self.search_results = result.unwrap_or_default();
        self.rebuild(cx);
    }
    pub(super) fn open_selected(&self, cx: &mut Context<Self>) {
        if !self.ready {
            return;
        }
        let Some(id) = &self.selected else {
            return;
        };
        if self.search_results.iter().any(|r| r.session_id == *id) {
            cx.emit(SidebarIntent::OpenMatch {
                session_id: id.clone(),
                query: self.search.read(cx).value().trim().to_string(),
            });
        } else {
            cx.emit(SidebarIntent::Open(id.clone()));
        }
    }
}
