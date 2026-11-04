#[path = "tool_details.rs"]
mod tool_details;
use super::*;
use gpui_kit::component::marker::{Marker, MarkerContent, MarkerLoadingStyle};
#[path = "activity.rs"]
mod activity;
/// Persistent assistant card; expansion state belongs to this UI turn only.
pub struct TurnCard {
    store: Entity<SessionStore>,
    pub(super) row: usize,
    cache: std::rc::Weak<RefCell<Cache>>,
    scroll: Entity<MessageScrollerState>,
    expanded: Option<bool>,
    tools: HashSet<String>,
    outputs: HashMap<String, (Entity<crate::tools::ToolOutputView>, Vec<Subscription>)>,
    collapsed_groups: HashSet<String>,
    refresh_outputs: bool,
    timer: Option<Task<()>>,
}
impl TurnCard {
    pub(super) fn new(
        store: Entity<SessionStore>,
        row: usize,
        cache: Rc<RefCell<Cache>>,
        scroll: Entity<MessageScrollerState>,
    ) -> Self {
        Self {
            store,
            row,
            cache: Rc::downgrade(&cache),
            scroll,
            expanded: None,
            tools: HashSet::new(),
            outputs: HashMap::new(),
            collapsed_groups: HashSet::new(),
            refresh_outputs: false,
            timer: None,
        }
    }
    pub(super) fn invalidate_outputs(&mut self) {
        self.refresh_outputs = true;
    }
    fn remeasure(&self, cx: &mut App) {
        self.scroll
            .update(cx, |s, cx| s.remeasure_items(self.row..self.row + 1, cx));
    }
}
impl EventEmitter<ChatIntent> for TurnCard {}
/// Source Markdown for explicit copy/details actions; no formatting is logged.
pub fn turn_markdown(turn: &AssistantTurn) -> String {
    turn.activities
        .iter()
        .filter(|a| turn.response.as_ref().and_then(|r| r.message_id.as_ref()) != Some(&a.id))
        .filter_map(|a| a.content.as_deref())
        .chain(turn.response.iter().map(|r| r.text.as_str()))
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}
impl Render for TurnCard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.refresh_outputs {
            for id in self.tools.iter().cloned().collect::<Vec<_>>() {
                self.create_output(&id, window, cx);
            }
            self.refresh_outputs = false;
        }
        let Some(cache) = self.cache.upgrade() else {
            return div().into_any_element();
        };
        let tool_active = self.store.read(cx).model.turns.get(self.row).is_some_and(
            |t| matches!(t,Turn::Assistant(t) if derive_turn_phase(t)==TurnPhase::ToolActive),
        );
        if tool_active && self.timer.is_none() {
            self.timer=Some(cx.spawn(async move|weak,cx|loop{
                cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
                let keep=weak.update(cx,|this,cx|{
                    let keep=this.store.read(cx).model.turns.get(this.row).is_some_and(|t|matches!(t,Turn::Assistant(t) if derive_turn_phase(t)==TurnPhase::ToolActive));
                    if keep{cx.notify();}keep
                }).unwrap_or(false);
                if !keep{break;}
            }));
        } else if !tool_active {
            self.timer = None;
        }
        let model = &self.store.read(cx).model;
        let Some(Turn::Assistant(turn)) = model.turns.get(self.row) else {
            return div().into_any_element();
        };
        let phase = derive_turn_phase(turn);
        let key = get_assistant_turn_ui_key(turn, self.row);
        let activities: Vec<_> = turn
            .activities
            .iter()
            .filter(|a| turn.response.as_ref().and_then(|r| r.message_id.as_ref()) != Some(&a.id))
            .collect();
        let collapsible = phase == TurnPhase::Complete && activities.len() > 3;
        let open = !collapsible || self.expanded.unwrap_or(false);
        let mut body = v_flex().w_full().gap_3();
        if collapsible {
            let count = activities
                .iter()
                .filter(|a| a.kind == ActivityType::Tool)
                .count();
            let summary = t!(
                "ak.chat.activity_summary",
                tools = count,
                activities = activities.len()
            );
            let elapsed = activities
                .iter()
                .filter_map(|a| a.elapsed_seconds)
                .filter(|s| s.is_finite() && *s >= 0.)
                .max_by(f64::total_cmp);
            let summary = elapsed.map_or_else(
                || summary.clone(),
                |seconds| {
                    t!(
                        "ak.chat.activity_duration",
                        summary = summary,
                        duration = format_duration(seconds * 1000.)
                    )
                },
            );
            body = body.child(
                Button::new(SharedString::from(format!("activities:{key}")))
                    .ghost()
                    .label(summary)
                    .accessibility_label(t!("ak.chat.toggle_activities"))
                    .icon(if open {
                        IconName::ChevronDown
                    } else {
                        IconName::ChevronRight
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.expanded = Some(!this.expanded.unwrap_or(false));
                        this.remeasure(cx);
                        cx.notify();
                    })),
            );
        }
        if open {
            body = body.child(activity::render(
                activities,
                &cache,
                &self.tools,
                &self.outputs,
                &self.collapsed_groups,
                cx,
            ));
        }
        if matches!(phase, TurnPhase::Pending | TurnPhase::Awaiting) {
            body = body.child(
                div()
                    .id("turn-thinking")
                    .role(Role::Status)
                    .aria_label(t!("ak.chat.thinking"))
                    .child(
                        Marker::new()
                            .loading(true)
                            .with_loading_style(MarkerLoadingStyle::Shimmer)
                            .content(MarkerContent::new().text(t!("ak.chat.thinking"))),
                    )
                    .test_support(),
            );
        }
        body = body.child(crate::permissions::render_notices(
            self.store.read(cx),
            Some(self.row),
            cx,
        ));
        if let Some(response) = &turn.response
            && let Some(id) = &response.message_id
            && let Some((_, view, _)) = cache.borrow().markdown.get(id)
        {
            body = body.child(view.clone());
        }
        if phase == TurnPhase::Streaming {
            body = body.child(
                div()
                    .id("turn-streaming")
                    .role(Role::Status)
                    .aria_label(t!("ak.chat.streaming"))
                    .child(div().w_1().h_4().bg(cx.theme().foreground).with_animation(
                        "stream-cursor",
                        Animation::new(std::time::Duration::from_millis(900)).repeat(),
                        |v, delta| v.opacity(if delta < 0.5 { 1.0 } else { 0.2 }),
                    ))
                    .test_support(),
            );
        }
        if phase == TurnPhase::Complete {
            let copy = key.clone();
            let details = key.clone();
            body = body.child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new(SharedString::from(format!("copy:{key}")))
                            .ghost()
                            .label(t!("ak.chat.copy"))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(ChatIntent::CopyMessage(copy.clone()))
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("details:{key}")))
                            .ghost()
                            .label(t!("ak.chat.details"))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(ChatIntent::ViewTurn(details.clone()))
                            })),
                    ),
            );
        }
        body.into_any_element()
    }
}
