use super::*;
use crate::tools::{ToolCard, ToolOutputView};
type Outputs = HashMap<String, (Entity<ToolOutputView>, Vec<Subscription>)>;
pub(super) fn render(
    activities: Vec<&Activity>,
    cache: &Rc<RefCell<Cache>>,
    expanded: &HashSet<String>,
    outputs: &Outputs,
    collapsed: &HashSet<String>,
    cx: &Context<TurnCard>,
) -> AnyElement {
    let grouped = crate::tools::grouped(&activities);
    let originals: HashMap<_, _> = activities.iter().map(|a| (a.id.as_str(), *a)).collect();
    let mut area = v_flex().gap_2().w_full();
    let mut seen = HashSet::new();
    for item in grouped {
        match item {
            ActivityOrGroup::Activity(a) => {
                if seen.insert(a.id.clone()) {
                    let canonical = originals.get(a.id.as_str()).copied().unwrap_or(&a);
                    area = area.child(row(
                        canonical,
                        a.depth.unwrap_or(0).max(0) as usize,
                        cache,
                        expanded,
                        outputs,
                        cx,
                    ));
                }
            }
            ActivityOrGroup::Group(group) => {
                let id = group.parent.id.clone();
                let hidden = collapsed.contains(&id);
                if seen.insert(id.clone()) {
                    let canonical = originals.get(id.as_str()).copied();
                    if let Some(canonical) = canonical {
                        area = area.child(row(canonical, 0, cache, expanded, outputs, cx));
                    } else {
                        let toggle = id.clone();
                        area = area.child(ToolCard::new(&group.parent, !hidden).on_toggle(
                            cx.listener(move |this, _, _, cx| {
                                if !this.collapsed_groups.remove(&toggle) {
                                    this.collapsed_groups.insert(toggle.clone());
                                }
                                this.remeasure(cx);
                                cx.notify();
                            }),
                        ));
                    }
                }
                let toggle = id.clone();
                area = area.child(
                    Button::new(SharedString::from(format!("tool-children:{id}")))
                        .ghost()
                        .label(t!("ak.tools.children", count = group.children.len()))
                        .icon(if hidden {
                            IconName::ChevronRight
                        } else {
                            IconName::ChevronDown
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if !this.collapsed_groups.remove(&toggle) {
                                this.collapsed_groups.insert(toggle.clone());
                            }
                            this.remeasure(cx);
                            cx.notify();
                        })),
                );
                if let Some(data) = group.task_output_data {
                    area = area.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(t!(
                                "ak.tools.usage",
                                input = format_tokens(data.input_tokens.unwrap_or(0.)),
                                output = format_tokens(data.output_tokens.unwrap_or(0.))
                            )),
                    );
                }
                for child in group.children {
                    if !seen.insert(child.id.clone()) {
                        continue;
                    }
                    if !hidden {
                        let canonical = originals.get(child.id.as_str()).copied().unwrap_or(&child);
                        area = area.child(row(
                            canonical,
                            1 + child.depth.unwrap_or(0).max(0) as usize,
                            cache,
                            expanded,
                            outputs,
                            cx,
                        ));
                    }
                }
            }
        }
    }
    // Malformed cyclic parent chains must not silently remove actual activities.
    for activity in activities {
        if activity.tool_name.as_deref() != Some("TaskOutput") && !seen.contains(&activity.id) {
            area = area.child(row(
                activity,
                activity.depth.unwrap_or(0).max(0) as usize,
                cache,
                expanded,
                outputs,
                cx,
            ));
        }
    }
    area.into_any_element()
}
fn row(
    activity: &Activity,
    depth: usize,
    cache: &Rc<RefCell<Cache>>,
    expanded: &HashSet<String>,
    outputs: &Outputs,
    cx: &Context<TurnCard>,
) -> AnyElement {
    if activity.kind == ActivityType::Intermediate {
        return cache
            .borrow()
            .markdown
            .get(&activity.id)
            .map(|(_, view, _)| view.clone().into_any_element())
            .unwrap_or_else(|| div().into_any_element());
    }
    if matches!(activity.kind, ActivityType::Status | ActivityType::Thinking) {
        return Marker::new()
            .content(MarkerContent::new().text(activity.content.clone().unwrap_or_default()))
            .into_any_element();
    }
    let id = activity.id.clone();
    let open = expanded.contains(&id);
    let mut card = ToolCard::new(activity, open)
        .depth(depth)
        .on_toggle(cx.listener(move |this, _, window, cx| this.toggle_tool(&id, window, cx)));
    if let Some((view, _)) = outputs.get(&activity.id) {
        card = card.content(view.clone());
    }
    card.into_any_element()
}
