use super::*;
use gpui_kit::component::spinner::Spinner;
pub(super) fn render(
    activities: Vec<&Activity>,
    cache: &Rc<RefCell<Cache>>,
    expanded: &HashSet<String>,
    cx: &Context<TurnCard>,
) -> AnyElement {
    let mut area = v_flex().gap_2().w_full();
    for activity in activities {
        if activity.kind == ActivityType::Intermediate {
            if let Some((_, view, _)) = cache.borrow().markdown.get(&activity.id) {
                area = area.child(view.clone());
            }
            continue;
        }
        if matches!(activity.kind, ActivityType::Status | ActivityType::Thinking) {
            if let Some(content) = &activity.content {
                area =
                    area.child(Marker::new().content(MarkerContent::new().text(content.clone())));
            }
            continue;
        }
        let id = activity.id.clone();
        let tool_open = expanded.contains(&id);
        let status = match activity.status {
            ActivityStatus::Error => t!("common.failed"),
            ActivityStatus::Completed => t!("ak.tool_done"),
            ActivityStatus::Backgrounded => t!("ak.chat.background"),
            _ => t!("ak.tool_running"),
        };
        let name = activity
            .display_name
            .clone()
            .or(activity.tool_name.clone())
            .unwrap_or_else(|| t!("ak.plan"));
        let mut heading = h_flex().gap_2().items_center();
        if activity.status == ActivityStatus::Running {
            heading = heading.child(Spinner::new());
        }
        heading = heading.child(
            Button::new(SharedString::from(format!("tool:{id}")))
                .ghost()
                .label(format!("{name} · {status}"))
                .accessibility_label(t!("ak.chat.toggle_tool"))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !this.tools.remove(&id) {
                        this.tools.insert(id.clone());
                    }
                    this.remeasure(cx);
                    cx.notify();
                })),
        );
        if let Some(intent) = &activity.intent {
            heading = heading.child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(intent.clone()),
            );
        }
        let seconds = activity.elapsed_seconds.or_else(|| {
            (activity.status == ActivityStatus::Running).then(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0., |d| {
                        ((d.as_millis() as i128 - i128::from(activity.timestamp)).max(0) as f64)
                            / 1000.
                    })
            })
        });
        if let Some(seconds) = seconds {
            heading = heading.child(format_duration(seconds * 1000.));
        }
        let mut tool = v_flex()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .child(heading);
        if tool_open {
            tool = tool
                .child(serde_json::to_string(&activity.tool_input).unwrap_or_default())
                .child(
                    activity
                        .content
                        .as_deref()
                        .unwrap_or_default()
                        .chars()
                        .take(2000)
                        .collect::<String>(),
                );
        }
        area = area.child(tool);
    }
    area.into_any_element()
}
