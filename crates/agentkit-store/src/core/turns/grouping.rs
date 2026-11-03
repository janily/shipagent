use super::*;
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (getToolStatus).
fn tool_status(m: &g::Message) -> ActivityStatus {
    if m.error_code.as_deref() == Some("response_too_large") {
        ActivityStatus::Completed
    } else if m.is_error == Some(true) {
        ActivityStatus::Error
    } else if m.tool_status == Some(g::ToolStatus::Backgrounded) {
        ActivityStatus::Backgrounded
    } else if m.tool_status == Some(g::ToolStatus::Completed) || m.tool_result.is_some() {
        ActivityStatus::Completed
    } else if m.tool_status == Some(g::ToolStatus::Pending) {
        ActivityStatus::Pending
    } else {
        ActivityStatus::Running
    }
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (stripErrorTags).
fn strip_error_tags(content: &str) -> String {
    // ASCII tag matching is case-insensitive, exactly like the upstream /gi expressions.
    fn remove(content: &str, tags: &[&str]) -> String {
        let mut result = String::new();
        let mut rest = content;
        while !rest.is_empty() {
            if let Some(tag) = tags.iter().find(|tag| {
                rest.get(..tag.len())
                    .is_some_and(|s| s.eq_ignore_ascii_case(tag))
            }) {
                rest = &rest[tag.len()..];
            } else if let Some(c) = rest.chars().next() {
                result.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
        result
    }
    let mut result = remove(
        &remove(content, &["<error>", "</error>"]),
        &["<tool_use_error>", "</tool_use_error>"],
    );
    if result
        .get(..7)
        .is_some_and(|s| s.eq_ignore_ascii_case("[ERROR]"))
    {
        result = result[7..].trim_start_matches(js_space).to_owned();
    }
    result.trim_matches(js_space).to_owned()
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (messageToActivity).
fn message_to_activity(m: &g::Message) -> Activity {
    let mut a = Activity::new(
        m.id.clone(),
        ActivityType::Tool,
        tool_status(m),
        m.timestamp,
    );
    a.tool_name = m.tool_name.clone();
    a.tool_use_id = m.tool_use_id.clone();
    a.tool_input = m.tool_input.clone();
    let content = nonempty(m.tool_result.as_deref()).unwrap_or(&m.content);
    a.content = Some(content.into());
    a.intent = m.tool_intent.clone();
    a.display_name = m.tool_display_name.clone();
    a.tool_display_meta = m.tool_display_meta.clone();
    a.error = (m.is_error == Some(true)).then(|| strip_error_tags(content));
    a.parent_id = m.parent_tool_use_id.clone();
    a.task_id = m.task_id.clone();
    a.shell_id = m.shell_id.clone();
    a.elapsed_seconds = m.elapsed_seconds;
    a.is_background = m.is_background;
    a
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (calculateActivityDepths).
fn calculate_depths(activities: &mut [Activity]) {
    let parents: HashMap<_, _> = activities
        .iter()
        .filter_map(|a| {
            nonempty(a.tool_use_id.as_deref()).map(|id| (id.to_owned(), a.parent_id.clone()))
        })
        .collect();
    for a in activities {
        let mut depth = 0;
        let mut parent = nonempty(a.parent_id.as_deref());
        while let Some(id) = parent {
            if depth == 10 {
                break;
            }
            depth += 1;
            parent = parents.get(id).and_then(|p| nonempty(p.as_deref()));
        }
        a.depth = Some(depth);
    }
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (extractTodosFromActivities).
fn extract_todos(activities: &[Activity]) -> Option<Vec<TodoItem>> {
    // Stable ordering retains the first activity at equal timestamps, as Array.sort does.
    let mut candidates: Vec<_> = activities
        .iter()
        .filter(|a| {
            a.tool_name.as_deref() == Some("TodoWrite")
                && a.status == ActivityStatus::Completed
                && nonempty(a.content.as_deref()).is_some()
        })
        .collect();
    candidates.sort_by_key(|a| std::cmp::Reverse(a.timestamp));
    let todos = candidates
        .first()?
        .tool_input
        .as_ref()?
        .get("todos")?
        .as_array()?;
    // Tool input is untrusted JSON. Invalid todo view-model rows are not rendered.
    serde_json::from_value(serde_json::Value::Array(todos.clone())).ok()
}
fn open(m: &g::Message, streaming: bool) -> AssistantTurn {
    AssistantTurn {
        kind: AssistantTurnKind::Assistant,
        turn_id: nonempty(m.turn_id.as_deref()).unwrap_or(&m.id).into(),
        activities: Vec::new(),
        response: None,
        intent: None,
        is_streaming: streaming,
        is_complete: false,
        timestamp: m.timestamp,
        todos: None,
    }
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (flushCurrentTurn).
fn flush(current: &mut Option<AssistantTurn>, turns: &mut Vec<Turn>, interrupted: bool) {
    if let Some(mut t) = current.take() {
        t.activities.sort_by_key(|a| a.timestamp);
        calculate_depths(&mut t.activities);
        t.todos = extract_todos(&t.activities);
        if interrupted {
            for a in &mut t.activities {
                if a.status == ActivityStatus::Running {
                    a.status = ActivityStatus::Error;
                    a.error = Some("Interrupted".into());
                }
            }
            if let Some(todos) = &mut t.todos {
                for todo in todos {
                    if todo.status == "in_progress" {
                        todo.status = "interrupted".into();
                    }
                }
            }
            t.is_streaming = false;
            t.is_complete = true;
        }
        if !interrupted
            && t.response.is_none()
            && t.is_complete
            && !t.activities.iter().any(|a| a.kind == ActivityType::Plan)
            && let Some(last) = t.activities.iter().rev().find(|a| {
                a.kind == ActivityType::Intermediate && nonempty(a.content.as_deref()).is_some()
            })
        {
            t.response = Some(ResponseContent {
                text: last.content.clone().unwrap_or_default(),
                is_streaming: false,
                message_id: Some(last.id.clone()),
                stream_start_time: None,
                is_plan: None,
                annotations: None,
            });
        }
        turns.push(Turn::Assistant(t));
    }
}
/// Ported from mkagent v0.2.6 packages/ui/src/components/chat/turn-utils.ts (groupMessagesByTurn).
/// Stable timestamp order and user/final/plan boundaries determine cards, not SDK turn IDs.
pub fn group_messages_by_turn(messages: &[g::Message], options: GroupTurnsOptions) -> Vec<Turn> {
    let mut sorted: Vec<_> = messages.iter().filter(|m| m.hidden != Some(true)).collect();
    sorted.sort_by_key(|m| m.timestamp);
    let mut turns = Vec::new();
    let mut current: Option<AssistantTurn> = None;
    for m in sorted {
        use g::MessageRole as R;
        match m.role {
            R::User => {
                if let Some(t) = &mut current {
                    t.is_complete = true;
                }
                flush(&mut current, &mut turns, false);
                turns.push(Turn::User(UserTurn {
                    kind: UserTurnKind::User,
                    message: m.clone(),
                    timestamp: m.timestamp,
                }));
            }
            R::Status => {
                let t = current.get_or_insert_with(|| {
                    let mut t = open(m, true);
                    t.turn_id = m.id.clone();
                    t
                });
                let mut a = Activity::new(
                    m.id.clone(),
                    ActivityType::Status,
                    ActivityStatus::Running,
                    m.timestamp,
                );
                a.content = Some(m.content.clone());
                a.status_type = m.status_type.as_ref().map(ToString::to_string);
                a.depth = Some(0);
                t.activities.push(a);
            }
            R::Info if m.status_type == Some(g::MessageStatusType::CompactionComplete) => {
                if let Some(t) = &mut current
                    && let Some(a) = t.activities.iter_mut().find(|a| {
                        a.kind == ActivityType::Status
                            && a.status_type.as_deref() == Some("compacting")
                    })
                {
                    a.status = ActivityStatus::Completed;
                    a.content = Some(m.content.clone());
                }
            }
            R::Error | R::Info | R::Warning => {
                let interrupted = m.role == R::Info;
                if let Some(t) = &mut current
                    && !interrupted
                {
                    t.is_complete = true;
                }
                flush(&mut current, &mut turns, interrupted);
                turns.push(Turn::System(SystemTurn {
                    kind: SystemTurnKind::System,
                    message: m.clone(),
                    timestamp: m.timestamp,
                }));
            }
            R::Plan => {
                let t = current.get_or_insert_with(|| open(m, false));
                let mut a = Activity::new(
                    m.id.clone(),
                    ActivityType::Plan,
                    ActivityStatus::Completed,
                    m.timestamp,
                );
                a.content = Some(m.content.clone());
                a.message_id = Some(m.id.clone());
                a.annotations = m.annotations.clone();
                a.display_name = Some("Plan".into());
                t.activities.push(a);
                t.is_streaming = false;
                t.is_complete = true;
                flush(&mut current, &mut turns, false);
            }
            R::Tool => {
                let complete = (m.tool_status == Some(g::ToolStatus::Completed)
                    || m.tool_result.is_some())
                    && m.tool_status != Some(g::ToolStatus::Backgrounded);
                let t = current.get_or_insert_with(|| {
                    let mut t = open(m, !complete);
                    t.intent = m.tool_intent.clone();
                    t
                });
                t.activities.push(message_to_activity(m));
                t.is_streaming = !complete;
            }
            R::Assistant => {
                if m.is_intermediate == Some(true) || m.is_pending == Some(true) {
                    let t = current.get_or_insert_with(|| open(m, m.is_pending == Some(true)));
                    let mut a = Activity::new(
                        m.id.clone(),
                        ActivityType::Intermediate,
                        if m.is_pending == Some(true) {
                            ActivityStatus::Running
                        } else {
                            ActivityStatus::Completed
                        },
                        m.timestamp,
                    );
                    a.content = Some(m.content.clone());
                    a.parent_id = m.parent_tool_use_id.clone();
                    t.activities.push(a);
                    if m.is_pending != Some(true) && m.is_streaming != Some(true) {
                        t.is_streaming = false;
                    }
                } else {
                    let t = current.get_or_insert_with(|| open(m, m.is_streaming == Some(true)));
                    t.response = Some(ResponseContent {
                        text: m.content.clone(),
                        is_streaming: m.is_streaming == Some(true),
                        stream_start_time: if m.is_streaming == Some(true) {
                            Some(m.timestamp)
                        } else {
                            None
                        },
                        message_id: Some(m.id.clone()),
                        annotations: m.annotations.clone(),
                        is_plan: None,
                    });
                    t.is_streaming = m.is_streaming == Some(true);
                    t.is_complete = !t.is_streaming;
                    if !t.is_streaming {
                        flush(&mut current, &mut turns, false);
                    }
                }
            }
        }
    }
    if options.is_session_processing == Some(false)
        && let Some(t) = &mut current
        && !t.activities.is_empty()
    {
        t.is_complete = true;
    }
    flush(&mut current, &mut turns, false);
    turns
}
