use super::*;
impl ChatView {
    pub(super) fn sync_count(&mut self, cx: &mut Context<Self>) {
        let count = self
            .store
            .as_ref()
            .map_or(0, |s| s.read(cx).model.turns.len());
        let old = self.scroll.read(cx).item_count();
        if count > old {
            self.scroll.update(cx, |s, cx| s.append(count - old, cx));
        } else if count < old {
            self.scroll.update(cx, |s, cx| s.splice(count..old, 0, cx));
        }
    }
    pub(super) fn range(&self, cx: &App) -> std::ops::Range<usize> {
        let scroll = self.scroll.read(cx);
        let center = if scroll.is_following_tail() {
            scroll.item_count().saturating_sub(1)
        } else {
            scroll.logical_scroll_top().item_ix
        };
        center.saturating_sub(200)..center.saturating_add(201)
    }
    pub(super) fn prune(&self, cx: &App) {
        let range = self.range(cx);
        let mut cache = self.cache.borrow_mut();
        cache.markdown.retain(|_, (row, _, _)| range.contains(row));
        cache.cards.retain(|_, (row, _, _)| range.contains(row));
    }
    pub(super) fn reconcile(&mut self, from: usize, cx: &mut Context<Self>) {
        let Some(store) = self.store.clone() else {
            return;
        };
        self.sync_count(cx);
        let model = &store.read(cx).model;
        let keys: HashMap<_, _> = model
            .turns
            .iter()
            .enumerate()
            .filter_map(|(row, t)| {
                if let Turn::Assistant(t) = t {
                    Some((get_assistant_turn_ui_key(t, row), row))
                } else {
                    None
                }
            })
            .collect();
        let moved: Vec<_> = {
            let mut cache = self.cache.borrow_mut();
            cache.cards.retain(|key, _| keys.contains_key(key));
            cache
                .cards
                .iter_mut()
                .filter_map(|(key, (row, card, _))| {
                    let next = keys[key];
                    if *row != next {
                        *row = next;
                        Some((card.clone(), next))
                    } else {
                        None
                    }
                })
                .collect()
        };
        for (card, row) in moved {
            card.update(cx, |card, _| card.row = row);
        }
        let model = &store.read(cx).model;
        let texts: Vec<_> = model
            .turns
            .iter()
            .enumerate()
            .skip(from)
            .filter(|(row, _)| self.range(cx).contains(row))
            .flat_map(|(row, turn)| {
                documents(turn)
                    .into_iter()
                    .map(move |(id, text, streaming)| (row, id, text, streaming))
            })
            .collect();
        let retained: HashSet<_> = model
            .state
            .session
            .messages
            .iter()
            .filter(|m| m.hidden != Some(true))
            .map(|m| m.id.clone())
            .collect();
        self.cache
            .borrow_mut()
            .markdown
            .retain(|id, _| retained.contains(id));
        let range = self.range(cx);
        for (row, id, text, streaming) in texts {
            if range.contains(&row) {
                let view = markdown(
                    &self.cache,
                    &id,
                    row,
                    &text,
                    streaming,
                    cx.weak_entity(),
                    cx,
                );
                view.update(cx, |v, cx| v.replace(&text, streaming, cx));
            }
        }
        let count = store.read(cx).model.turns.len();
        let old = self.scroll.read(cx).item_count();
        let start = from.min(old).min(count);
        self.scroll.update(cx, |s, cx| {
            if start < old || start < count {
                s.splice(start..old, count - start, cx);
            }
            if start < count {
                s.remeasure_items(start..count, cx);
            }
        });
        // Suffix regrouping may change row identities. Existing card objects read
        // their current canonical Turn by index; stale keys are dropped on render.
        for (_, view, _) in self
            .cache
            .borrow()
            .cards
            .values()
            .filter(|(row, _, _)| *row >= from)
        {
            view.update(cx, |card, cx| {
                card.invalidate_outputs();
                cx.notify();
            });
        }
        self.prune(cx);
        cx.notify();
    }
}
pub(super) fn documents(turn: &Turn) -> Vec<(String, String, bool)> {
    match turn {
        Turn::Assistant(t) => assistant_documents(t),
        _ => Vec::new(),
    }
}
pub(super) fn assistant_documents(t: &AssistantTurn) -> Vec<(String, String, bool)> {
    t.activities
        .iter()
        .filter(|a| a.kind == ActivityType::Intermediate)
        .filter_map(|a| {
            a.content.as_ref().map(|text| {
                (
                    a.id.clone(),
                    text.clone(),
                    a.status == ActivityStatus::Running,
                )
            })
        })
        .chain(t.response.iter().filter_map(|r| {
            r.message_id
                .as_ref()
                .map(|id| (id.clone(), r.text.clone(), r.is_streaming))
        }))
        .collect()
}
pub(super) fn markdown(
    cache: &Rc<RefCell<Cache>>,
    id: &str,
    row: usize,
    text: &str,
    streaming: bool,
    parent: WeakEntity<ChatView>,
    cx: &mut App,
) -> Entity<MarkdownView> {
    if let Some((position, view, _)) = cache.borrow_mut().markdown.get_mut(id) {
        *position = row;
        return view.clone();
    }
    let view = cx.new(|cx| MarkdownView::new(text, streaming, cx));
    let subscription = cx.subscribe(&view, move |_, event: &ChatIntent, cx| {
        let _ = parent.update(cx, |_, cx| cx.emit(event.clone()));
    });
    cache
        .borrow_mut()
        .markdown
        .insert(id.into(), (row, view.clone(), subscription));
    view
}
