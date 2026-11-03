use super::*;
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (computeLastChildSet).
pub fn compute_last_child_set(activities: &[Activity]) -> HashSet<String> {
    let mut last = HashMap::new();
    for a in activities {
        if a.depth.is_some_and(|d| d > 0) {
            last.insert(a.parent_id.as_deref(), a.id.clone());
        }
    }
    last.into_values().collect()
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (isActivityGroup).
pub fn is_activity_group(item: &ActivityOrGroup) -> bool {
    matches!(item, ActivityOrGroup::Group(_))
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (countTotalActivities).
pub fn count_total_activities(items: &[ActivityOrGroup]) -> usize {
    items
        .iter()
        .map(|i| match i {
            ActivityOrGroup::Group(g) => 1 + g.children.len(),
            ActivityOrGroup::Activity(_) => 1,
        })
        .sum()
}
fn parent_tool(name: Option<&str>) -> bool {
    matches!(name, Some("Task" | "Agent"))
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (extractTaskOutputData).
fn task_output(a: &Activity) -> Option<TaskOutputData> {
    let parsed: serde_json::Value = serde_json::from_str(a.content.as_deref()?).ok()?;
    let data = TaskOutputData {
        duration_ms: parsed["duration_ms"].as_f64(),
        input_tokens: parsed["usage"]["input_tokens"].as_f64(),
        output_tokens: parsed["usage"]["output_tokens"].as_f64(),
    };
    if data.duration_ms.is_some() || data.input_tokens.is_some() || data.output_tokens.is_some() {
        Some(data)
    } else {
        None
    }
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (groupActivitiesByParent).
pub fn group_activities_by_parent(activities: &[Activity]) -> Vec<ActivityOrGroup> {
    let by_tool: HashMap<_, _> = activities
        .iter()
        .filter_map(|a| nonempty(a.tool_use_id.as_deref()).map(|id| (id, a)))
        .collect();
    let group_key = |a: &Activity| {
        let mut key = None;
        let mut parent = nonempty(a.parent_id.as_deref());
        for _ in 0..10 {
            let Some(id) = parent else { break };
            let Some(p) = by_tool.get(id) else {
                return Some(format!("missing:{id}"));
            };
            if parent_tool(p.tool_name.as_deref()) {
                key = Some(id.to_owned());
            }
            parent = nonempty(p.parent_id.as_deref());
        }
        key
    };
    let mut descendants: HashMap<String, Vec<Activity>> = HashMap::new();
    let mut key_by_id = HashMap::new();
    for a in activities {
        if a.tool_name.as_deref() == Some("TaskOutput") {
            continue;
        }
        if let Some(key) = group_key(a) {
            key_by_id.insert(&a.id, key.clone());
            descendants.entry(key).or_default().push(a.clone());
        }
    }
    let outputs: HashMap<_, _> = activities
        .iter()
        .filter(|a| {
            a.tool_name.as_deref() == Some("TaskOutput") && a.status == ActivityStatus::Completed
        })
        .filter_map(|a| {
            let id = a
                .tool_input
                .as_ref()?
                .get("task_id")?
                .as_str()
                .filter(|s| !s.is_empty())?;
            Some((id, task_output(a)?))
        })
        .collect();
    let mut agent_ids = HashMap::new();
    for a in activities {
        if parent_tool(a.tool_name.as_deref())
            && matches!(
                a.status,
                ActivityStatus::Completed | ActivityStatus::Backgrounded
            )
            && let Some(content) = &a.content
        {
            let id = content.match_indices("agentId:").find_map(|(start, _)| {
                let id: String = content[start + 8..]
                    .trim_start_matches(js_space)
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
                    .collect();
                if id.is_empty() { None } else { Some(id) }
            });
            if let Some(id) = id
                && let Some(tool) = nonempty(a.tool_use_id.as_deref())
            {
                agent_ids.insert(tool, id);
            }
        }
    }
    let children = |key: &str, base: i32| {
        let mut list = descendants.get(key).cloned().unwrap_or_default();
        list.sort_by_key(|c| c.timestamp);
        for c in &mut list {
            c.depth = Some((c.depth.unwrap_or(0) - base - 1).max(0));
        }
        list
    };
    let mut result = Vec::new();
    let mut continued = HashSet::new();
    for a in activities {
        if a.tool_name.as_deref() == Some("TaskOutput") {
            continue;
        }
        if let Some(key) = key_by_id.get(&a.id) {
            if let Some(id) = key.strip_prefix("missing:")
                && continued.insert(key)
            {
                let list = children(key, 0);
                let status = if list
                    .iter()
                    .any(|c| matches!(c.status, ActivityStatus::Running | ActivityStatus::Pending))
                {
                    ActivityStatus::Running
                } else if list.iter().any(|c| c.status == ActivityStatus::Error) {
                    ActivityStatus::Error
                } else {
                    ActivityStatus::Completed
                };
                let mut parent = Activity::new(
                    format!("continued:{id}"),
                    ActivityType::Tool,
                    status,
                    a.timestamp,
                );
                parent.tool_name = Some("Agent".into());
                parent.tool_use_id = Some(id.into());
                parent.depth = Some(0);
                result.push(ActivityOrGroup::Group(ActivityGroup {
                    kind: ActivityGroupKind::Group,
                    parent,
                    children: list,
                    continued: Some(true),
                    task_output_data: None,
                }));
            }
            continue;
        }
        if parent_tool(a.tool_name.as_deref())
            && let Some(tool) = nonempty(a.tool_use_id.as_deref())
        {
            let data = agent_ids
                .get(tool)
                .and_then(|id| outputs.get(id.as_str()))
                .cloned();
            result.push(ActivityOrGroup::Group(ActivityGroup {
                kind: ActivityGroupKind::Group,
                parent: a.clone(),
                children: children(tool, a.depth.unwrap_or(0)),
                continued: None,
                task_output_data: data,
            }));
        } else {
            result.push(ActivityOrGroup::Activity(a.clone()));
        }
    }
    result
}
