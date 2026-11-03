use super::*;
pub(super) fn render(
    index: usize,
    store: &Entity<SessionStore>,
    cache: &Rc<RefCell<Cache>>,
    scroll: Entity<MessageScrollerState>,
    parent: WeakEntity<ChatView>,
    cx: &mut App,
) -> AnyElement {
    let Some(turn) = store.read(cx).model.turns.get(index) else {
        return div().into_any_element();
    };
    let (label, content) = match turn {
        Turn::User(turn) => {
            let message = turn.message.clone();
            (
                message.content.clone(),
                super::user::render(message, parent.clone()).into_any_element(),
            )
        }
        Turn::System(turn) => {
            let message = turn.message.clone();
            (
                message.content.clone(),
                super::system::render(message, parent.clone(), cache, cx),
            )
        }
        Turn::Assistant(turn) => {
            let key = get_assistant_turn_ui_key(turn, index);
            let label = turn
                .response
                .as_ref()
                .map(|r| r.text.clone())
                .or_else(|| {
                    turn.activities
                        .iter()
                        .rev()
                        .find(|a| a.kind == ActivityType::Intermediate)
                        .and_then(|a| a.content.clone())
                })
                .unwrap_or_default();
            let missing: Vec<_> = assistant_documents(turn)
                .into_iter()
                .filter(|(id, _, _)| !cache.borrow().markdown.contains_key(id))
                .collect();
            for (id, text, streaming) in missing {
                markdown(cache, &id, index, &text, streaming, parent.clone(), cx);
            }
            let old = cache
                .borrow()
                .cards
                .get(&key)
                .map(|(_, card, _)| card.clone());
            let card = if let Some(card) = old {
                card.update(cx, |card, _| card.row = index);
                card
            } else {
                let card = cx.new(|_| TurnCard::new(store.clone(), index, cache.clone(), scroll));
                let parent = parent.clone();
                let subscription = cx.subscribe(&card, move |_, event: &ChatIntent, cx| {
                    emit(&parent, event.clone(), cx)
                });
                cache
                    .borrow_mut()
                    .cards
                    .insert(key, (index, card.clone(), subscription));
                card
            };
            (label, card.into_any_element())
        }
    };
    h_flex()
        .w_full()
        .justify_center()
        .px_6()
        .py_2()
        .child(
            div()
                .id(("message", index))
                .role(Role::Group)
                .aria_label(label)
                .w_full()
                .max_w(px(820.))
                .child(content)
                .test_support(),
        )
        .into_any_element()
}
