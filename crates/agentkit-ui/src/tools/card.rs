use crate::t;
use agentkit_store::core::turns::{Activity, ActivityStatus, format_duration};
use gpui_kit::component::*;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    spinner::Spinner,
};
use gpui_kit::{prelude::*, *};
use std::rc::Rc;
type ToggleHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
/// A summary-only tool card. Heavy native contents are supplied only when expanded.
#[derive(IntoElement)]
pub struct ToolCard {
    id: String,
    name: String,
    intent: Option<String>,
    status: ActivityStatus,
    denied: bool,
    seconds: Option<f64>,
    open: bool,
    depth: usize,
    body: Option<AnyElement>,
    toggle: Option<ToggleHandler>,
}
impl ToolCard {
    /// Snapshot bounded metadata without copying input or result payloads.
    pub fn new(activity: &Activity, open: bool) -> Self {
        let name = activity
            .display_name
            .as_deref()
            .or(activity
                .tool_display_meta
                .as_ref()
                .map(|m| m.display_name.as_str()))
            .or(activity.tool_name.as_deref())
            .map(|s| super::model::snippet(s, 1, 512).text)
            .unwrap_or_else(|| t!("ak.tools.unknown"));
        let denied = activity.status == ActivityStatus::Error
            && activity
                .error
                .as_deref()
                .or(activity.content.as_deref())
                .is_some_and(|s| {
                    s.as_bytes()[..s.len().min(512)]
                        .windows(b"permission denied by user".len())
                        .any(|part| part.eq_ignore_ascii_case(b"permission denied by user"))
                });
        let seconds = activity.elapsed_seconds.or_else(|| {
            (activity.status == ActivityStatus::Running).then(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0., |d| {
                        (d.as_millis() as i128 - i128::from(activity.timestamp)).max(0) as f64
                            / 1000.
                    })
            })
        });
        Self {
            id: activity.id.clone(),
            name,
            intent: activity
                .intent
                .as_deref()
                .map(|s| super::model::snippet(s, 1, 512).text),
            status: activity.status,
            denied,
            seconds,
            open,
            depth: 0,
            body: None,
            toggle: None,
        }
    }
    /// Indent a descendant using the Kit spacing scale, capped at four levels.
    pub fn depth(mut self, depth: usize) -> Self {
        self.depth = depth.min(4);
        self
    }
    /// Add prepared native details. A collapsed card never renders this element.
    pub fn content(mut self, body: impl IntoElement) -> Self {
        self.body = Some(body.into_any_element());
        self
    }
    /// Request a host-owned expansion change, usable by mouse and keyboard.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.toggle = Some(Rc::new(handler));
        self
    }
}
impl RenderOnce for ToolCard {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let (status, icon) = if self.denied {
            (t!("ak.tools.denied"), IconName::Ban)
        } else {
            match self.status {
                ActivityStatus::Completed => (t!("ak.tool_done"), IconName::CircleCheck),
                ActivityStatus::Error => (t!("common.failed"), IconName::CircleX),
                ActivityStatus::Backgrounded => (t!("ak.chat.background"), IconName::Info),
                ActivityStatus::Pending => (t!("ak.tools.pending"), IconName::Info),
                ActivityStatus::Running => (t!("ak.tool_running"), IconName::Info),
            }
        };
        let mut heading = h_flex().items_center().gap_2().w_full();
        heading = heading.child(
            Button::new(SharedString::from(format!("tool:{}", self.id)))
                .ghost()
                .icon(if self.open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .label(self.name.clone())
                .accessibility_label(t!("ak.tools.toggle", name = self.name, status = status))
                .on_click(move |event, window, cx| {
                    if let Some(toggle) = &self.toggle {
                        toggle(event, window, cx);
                    }
                }),
        );
        if let Some(intent) = self.intent {
            heading = heading.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(intent),
            );
        } else {
            heading = heading.child(div().flex_1());
        }
        if self.status == ActivityStatus::Running {
            heading = heading.child(Spinner::new());
        } else {
            heading = heading.child(Icon::new(icon).small());
        }
        heading = heading.child(div().text_xs().child(status));
        if let Some(seconds) = self.seconds {
            heading = heading.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format_duration(seconds * 1000.)),
            );
        }
        let mut card = v_flex()
            .id(SharedString::from(format!("tool-card:{}", self.id)))
            .role(Role::Group)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md();
        card = match self.depth {
            1 => card.ml_3(),
            2 => card.ml_6(),
            3 => card.ml_9(),
            4 => card.ml_12(),
            _ => card,
        };
        let body = self.body.unwrap_or_else(|| div().into_any_element());
        card.child(
            Collapsible::new()
                .open(self.open)
                .child(heading)
                .content(body),
        )
        .test_support()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn ordinary_successful_output_cannot_be_mistaken_for_a_denial() -> Result<(), serde_json::Error>
    {
        let mut activity: Activity = serde_json::from_value(
            serde_json::json!({"id":"read","type":"tool","status":"completed","timestamp":1,"toolName":"Read","content":"Permission denied by user. This is ordinary file content."}),
        )?;
        assert!(!ToolCard::new(&activity, false).denied);
        activity.status = ActivityStatus::Error;
        assert!(ToolCard::new(&activity, false).denied);
        Ok(())
    }
}
