#[path = "draft_loading.rs"]
mod loading;
use super::*;
use crate::draft_io::Snapshot;
use agentkit_protocol::generated as g;
use std::collections::HashMap;
#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    entity: EntityId,
    revision: u64,
    files: Vec<String>,
    unavailable: Vec<String>,
}
fn stamp(store: &Entity<SessionStore>, cx: &App) -> Stamp {
    let s = store.read(cx);
    Stamp {
        entity: store.entity_id(),
        revision: s.model.draft_revision(),
        files: s.attachments.files.iter().map(|f| f.id.clone()).collect(),
        unavailable: s
            .draft_sync
            .unavailable
            .iter()
            .map(|r| r.id.clone())
            .collect(),
    }
}
fn snapshot(store: &SessionStore) -> Snapshot {
    let mut references = store
        .draft_sync
        .unavailable
        .iter()
        .map(|r| r.reference.clone())
        .collect::<Vec<_>>();
    references.extend(store.model.draft_attachments.iter().map(|a| {
        std::sync::Arc::new(g::SessionDraftAttachmentsItem {
            path: a.stored_path.clone(),
            name: a.name.clone(),
            content: None,
        })
    }));
    Snapshot {
        text: store.model.draft.text.clone(),
        files: store
            .attachments
            .files
            .iter()
            .map(|f| f.file.clone())
            .collect(),
        references,
    }
}
struct Lane {
    stamp: Stamp,
    loaded: bool,
    loading: bool,
    load_sequence: u64,
    sequence: u64,
    _subscription: Subscription,
}
#[derive(Default)]
pub(super) struct Drafts {
    lanes: HashMap<String, Lane>,
}
/// Local draft data preserved when an authoritative session entity is rebuilt.
#[derive(Clone)]
pub(in crate::controller) struct LocalDraft {
    draft: g::SessionDraft,
    stored: Vec<g::StoredAttachment>,
    files: agentkit_store::core::attachments::Draft,
    unavailable: Vec<agentkit_store::core::drafts::Unavailable>,
}
impl LocalDraft {
    pub(in crate::controller) fn capture(s: &SessionStore) -> Self {
        Self {
            draft: s.model.draft.clone(),
            stored: s.model.draft_attachments.clone(),
            files: s.attachments.clone(),
            unavailable: s.draft_sync.unavailable.clone(),
        }
    }
    pub(in crate::controller) fn restore(mut self, s: &mut SessionStore) {
        s.model.draft = self.draft;
        s.model.draft_attachments = self.stored;
        self.files.reading = 0;
        self.files.sending = false;
        s.attachments = self.files;
        s.draft_sync.unavailable = self.unavailable;
    }
}
impl WorkbenchController {
    pub(super) fn bind_drafts(&mut self, api: WorkbenchApi, cx: &mut Context<Self>) {
        self.draft_writer.bind(api);
        for (id, lane) in &mut self.draft_sync.lanes {
            lane.load_sequence = lane.load_sequence.wrapping_add(1);
            lane.loading = false;
            if let Some(store) = self.stores.get(id) {
                store.update(cx, |s, cx| {
                    s.draft_sync.loading = false;
                    cx.notify();
                });
            }
        }
    }
    pub(super) fn prepare_draft_exit(&mut self, cx: &mut Context<Self>) {
        self.save_draft(cx);
        for store in self.stores.values().cloned().collect::<Vec<_>>() {
            self.persist_draft(&store, true, cx);
        }
    }
    pub(super) fn watch_draft(
        &mut self,
        store: Entity<SessionStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = store.read(cx).model.state.session.id.clone();
        let replace = self
            .draft_sync
            .lanes
            .get(&id)
            .is_none_or(|l| l.stamp.entity != store.entity_id());
        if replace {
            let loaded = self.draft_sync.lanes.get(&id).is_some_and(|l| l.loaded)
                || !store.read(cx).model.draft.text.is_empty()
                || !store.read(cx).attachments.files.is_empty()
                || !store.read(cx).model.draft_attachments.is_empty();
            let sub = cx.observe_in(&store, window, |this, store, _, cx| {
                this.persist_draft(&store, false, cx)
            });
            self.draft_sync.lanes.insert(
                id.clone(),
                Lane {
                    stamp: stamp(&store, cx),
                    loaded,
                    loading: false,
                    load_sequence: 0,
                    sequence: self.draft_writer.pending_sequence(&id).unwrap_or(0),
                    _subscription: sub,
                },
            );
        }
        self.load_draft(store, window, cx);
    }
    pub(super) fn persist_draft(
        &mut self,
        store: &Entity<SessionStore>,
        immediate: bool,
        cx: &mut Context<Self>,
    ) {
        let id = store.read(cx).model.state.session.id.clone();
        let next = stamp(store, cx);
        let Some(lane) = self.draft_sync.lanes.get_mut(&id) else {
            return;
        };
        if lane.stamp == next && (!immediate || self.draft_writer.pending_sequence(&id).is_none()) {
            return;
        }
        lane.stamp = next;
        lane.loaded = true;
        let data = snapshot(store.read(cx));
        let empty = data.text.is_empty() && data.files.is_empty() && data.references.is_empty();
        lane.sequence = self.draft_writer.save(id, data, immediate || empty);
        store.update(cx, |s, cx| {
            s.draft_sync.saving = true;
            s.draft_sync.error = None;
            cx.notify();
        });
    }
    pub(super) fn draft_notices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let notices = self.draft_writer.notices.clone();
        self.operations
            .push(cx.spawn_in(window, async move |weak, cx| {
                while let Ok(notice) = notices.recv().await {
                    let _ = weak.update_in(cx, |this, _, cx| {
                        if this
                            .draft_sync
                            .lanes
                            .get(&notice.session)
                            .is_none_or(|l| l.sequence != notice.sequence)
                        {
                            return;
                        }
                        if let Some(store) = this.stores.get(&notice.session) {
                            store.update(cx, |s, cx| {
                                s.draft_sync.saving = false;
                                s.draft_sync.error = notice.result.err();
                                cx.notify();
                            });
                        }
                    });
                }
            }));
    }
}

#[cfg(test)]
#[path = "draft_contract_tests.rs"]
mod tests;
