//! Native virtualized conversation views. These components emit intents only.
use crate::t;
use agentkit_store::{
    core::{session::SessionStoreEvent, turns::*},
    entities::SessionStore,
};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    message_scroller::{MessageScroller, MessageScrollerState},
    *,
};
use gpui_kit::{prelude::*, *};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};
mod cache;
use cache::*;
mod markdown;
mod rows;
mod system;
mod user;
pub(super) fn emit(parent: &WeakEntity<ChatView>, event: ChatIntent, cx: &mut App) {
    let _ = parent.update(cx, |_, cx| cx.emit(event));
}

mod turn;
pub use markdown::MarkdownView;
pub use turn::{TurnCard, turn_markdown};
/// Conversation actions handled by the application, never by RPC in the views.
#[derive(Clone, Debug)]
pub enum ChatIntent {
    /// Open original canonical tool content in a read-only native editor.
    OpenToolContent {
        /// Stable activity/message ID.
        activity_id: String,
        /// Original result, input packet or selected input field.
        content: crate::tools::ToolContent,
    },
    /// Copy a canonical message or completed assistant turn.
    CopyMessage(String),
    /// Show canonical assistant turn Markdown in a details dialog.
    ViewTurn(String),
    /// Open a Markdown link through application URL policy.
    LinkClicked(String),
    /// Open a local file through the application capability layer.
    OpenFile(String),
    /// Branch from a canonical user message.
    BranchFrom(String),
    /// Dispatch a declared action on an authoritative typed error.
    ErrorAction {
        /// Error message identity.
        message_id: String,
        /// Backend-declared action key.
        key: String,
    },
}
#[derive(Default)]
struct Cache {
    reduce_stream_fade: bool,
    markdown: HashMap<String, (usize, Entity<MarkdownView>, Subscription)>,
    details: HashSet<String>,
    cards: HashMap<String, (usize, Entity<TurnCard>, Subscription)>,
}
/// A SessionStore-backed virtual conversation; rows correspond exactly to Turns.
pub struct ChatView {
    store: Option<Entity<SessionStore>>,
    scroll: Entity<MessageScrollerState>,
    cache: Rc<RefCell<Cache>>,
    subscriptions: Vec<Subscription>,
}
impl EventEmitter<ChatIntent> for ChatView {}
impl ChatView {
    /// Construct an empty independent transcript.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let scroll = cx.new(|cx| MessageScrollerState::new(0, cx));
        Self {
            store: None,
            scroll,
            cache: Rc::default(),
            subscriptions: Vec::new(),
        }
    }
    /// Update existing and future Markdown documents in this chat instance.
    pub fn set_stream_fade(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let mut cache = self.cache.borrow_mut();
        cache.reduce_stream_fade = !enabled;
        let views: Vec<_> = cache.markdown.values().map(|(_, v, _)| v.clone()).collect();
        drop(cache);
        for view in views {
            view.update(cx, |v, cx| v.set_stream_fade(enabled, cx));
        }
        cx.notify();
    }
    /// Access virtual-list scroll policy for host actions and native tests.
    pub fn scroll_state(&self) -> &Entity<MessageScrollerState> {
        &self.scroll
    }
    /// Bind a session, releasing all previous subscriptions and rendering state.
    pub fn bind(&mut self, store: Entity<SessionStore>, cx: &mut Context<Self>) {
        self.clear(cx);
        self.store = Some(store.clone());
        self.reconcile(0, cx);
        self.subscriptions
            .push(cx.subscribe(&store, |this, store, event, cx| {
                match event {
                    SessionStoreEvent::TurnsChanged { from_turn } => this.reconcile(*from_turn, cx),
                    SessionStoreEvent::StreamDelta { message_id, delta } => {
                        let entry = this
                            .cache
                            .borrow()
                            .markdown
                            .get(message_id)
                            .map(|(_, v, _)| v.clone());
                        if let Some(view) = entry {
                            view.update(cx, |v, cx| v.append(delta, cx));
                        }
                        this.sync_count(cx);
                        if let Some(row) = store.read(cx).model.turn_index(message_id) {
                            this.scroll
                                .update(cx, |s, cx| s.remeasure_items(row..row + 1, cx));
                            for (_, card, _) in this
                                .cache
                                .borrow()
                                .cards
                                .values()
                                .filter(|(i, _, _)| *i == row)
                            {
                                card.update(cx, |_, cx| cx.notify());
                            }
                        }
                    }
                }
                cx.notify();
            }));
        self.subscriptions.push(cx.observe(&store, |this, _, cx| {
            this.sync_count(cx);
            cx.notify();
        }));
        let scroll = self.scroll.clone();
        self.subscriptions.push(cx.observe(&scroll, |this, _, cx| {
            this.prune(cx);
            cx.notify();
        }));
    }
    /// Release the active session and reset tail following.
    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.subscriptions.clear();
        self.store = None;
        let reduce_stream_fade = self.cache.borrow().reduce_stream_fade;
        *self.cache.borrow_mut() = Cache {
            reduce_stream_fade,
            ..Default::default()
        };
        self.scroll.update(cx, |s, cx| s.reset(0, cx));
        cx.notify();
    }
    /// Inspect a retained Markdown document in sample/test builds.
    #[cfg(feature = "samples")]
    pub fn cached_markdown(&self, id: &str, cx: &App) -> Option<String> {
        self.cache
            .borrow()
            .markdown
            .get(id)
            .map(|(_, v, _)| v.read(cx).text().to_owned())
    }
    /// Number of retained documents and turn entities, for cache-bound regressions.
    #[cfg(feature = "samples")]
    pub fn cache_sizes(&self) -> (usize, usize) {
        let c = self.cache.borrow();
        (c.markdown.len(), c.cards.len())
    }
}
impl Render for ChatView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(store) = self.store.clone() else {
            return div().into_any_element();
        };
        if store.read(cx).model.turns.is_empty() {
            return v_flex()
                .flex_1()
                .justify_center()
                .items_center()
                .child(t!("ak.empty_hint"))
                .into_any_element();
        }
        let cache = self.cache.clone();
        let parent = cx.weak_entity();
        let scroll = self.scroll.clone();
        MessageScroller::new("messages", self.scroll.clone(), move |index, _, cx| {
            rows::render(index, &store, &cache, scroll.clone(), parent.clone(), cx)
        })
        .with_jump_button_label(t!("ak.latest"))
        .flex_1()
        .min_h_0()
        .into_any_element()
    }
}
