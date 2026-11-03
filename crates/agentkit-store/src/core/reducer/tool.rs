//! Ported from mkagent v0.2.6 event-processor/handlers/tool.ts.
use super::*;
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/handlers/tool.ts.
pub(super) fn start(
    state: &mut SessionState,
    e: &g::SessionToolStartEvent,
    ctx: &Context,
) -> Result<(), ReducerError> {
    let index = tool_index(state, &e.tool_use_id);
    let mut m = if let Some(i) = index {
        state.session.messages[i].clone()
    } else {
        let mut m = message(ctx, g::MessageRole::Tool, String::new());
        m.timestamp = time(e.timestamp, ctx.now_ms)?;
        m.tool_use_id = Some(e.tool_use_id.clone());
        m.tool_name = Some(e.tool_name.clone());
        m.tool_status = Some(g::ToolStatus::Executing);
        m
    };
    m.tool_input = Some(e.tool_input.clone());
    m.tool_intent = e.tool_intent.clone();
    m.tool_display_name = e.tool_display_name.clone();
    m.tool_display_meta = convert(&e.tool_display_meta)?;
    m.turn_id = e.turn_id.clone();
    m.parent_tool_use_id = e.parent_tool_use_id.clone();
    if let Some(i) = index {
        state.session.messages[i] = m;
    } else {
        append(&mut state.session, m, false, ctx);
    }
    Ok(())
}
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/handlers/tool.ts.
pub(super) fn result(
    state: &mut SessionState,
    e: &g::SessionToolResultEvent,
    ctx: &Context,
) -> Result<(), ReducerError> {
    let index = tool_index(state, &e.tool_use_id);
    // Match upstream /^\s*/: ECMAScript includes BOM but excludes U+0085.
    let trimmed = e.result.trim_start_matches(|c| {
        matches!(c,
            '\u{0009}'..='\u{000d}' | ' ' | '\u{00a0}' | '\u{1680}' |
            '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' |
            '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
        )
    });
    let inferred = e.is_error == Some(true)
        || ["[ERROR]", "Error:", "error:"]
            .iter()
            .any(|p| trimmed.starts_with(p));
    let persisted = inferred
        && (e.result.contains("Output has been saved to")
            || e.result.contains("Full output saved to"));
    let error = inferred && !persisted;
    let mut m = if let Some(i) = index {
        state.session.messages[i].clone()
    } else {
        let mut m = message(ctx, g::MessageRole::Tool, String::new());
        m.timestamp = time(e.timestamp, ctx.now_ms)?;
        m.tool_use_id = Some(e.tool_use_id.clone());
        m.tool_name = Some(e.tool_name.clone());
        m.tool_input = e.tool_input.clone();
        m.turn_id = e.turn_id.clone();
        m.parent_tool_use_id = e.parent_tool_use_id.clone();
        m
    };
    let background = index.is_some()
        && (m.tool_status == Some(g::ToolStatus::Backgrounded) || m.is_background == Some(true));
    m.tool_result = Some(e.result.clone());
    m.tool_status = Some(if background {
        g::ToolStatus::Backgrounded
    } else if error {
        g::ToolStatus::Error
    } else {
        g::ToolStatus::Completed
    });
    m.is_error = Some(error);
    m.error_code = if persisted {
        Some("response_too_large".into())
    } else {
        None
    };
    if let Some(input) = &e.tool_input {
        m.tool_input = Some(input.clone());
    }
    if let Some(i) = index {
        let parent = matches!(
            m.tool_name.as_deref(),
            Some("Task" | "Agent" | "TaskOutput")
        );
        state.session.messages[i] = m;
        if parent {
            for child in &mut state.session.messages {
                if child.parent_tool_use_id.as_deref() == Some(&e.tool_use_id)
                    && !matches!(
                        child.tool_status,
                        Some(g::ToolStatus::Completed | g::ToolStatus::Error)
                    )
                {
                    child.tool_status = Some(g::ToolStatus::Completed);
                    if child.tool_result.as_ref().is_none_or(|s| s.is_empty()) {
                        child.tool_result = Some(String::new());
                    }
                }
            }
        }
    } else {
        append(&mut state.session, m, false, ctx);
    }
    Ok(())
}
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/handlers/tool.ts.
pub(super) fn task_backgrounded(state: &mut SessionState, e: &g::SessionTaskBackgroundedEvent) {
    if let Some(i) = tool_index(state, &e.tool_use_id) {
        let m = &mut state.session.messages[i];
        m.tool_status = Some(g::ToolStatus::Backgrounded);
        m.task_id = Some(e.task_id.clone());
        m.is_background = Some(true);
    }
}
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/handlers/tool.ts.
pub(super) fn shell_backgrounded(state: &mut SessionState, e: &g::SessionShellBackgroundedEvent) {
    if let Some(i) = tool_index(state, &e.tool_use_id) {
        let m = &mut state.session.messages[i];
        m.tool_status = Some(g::ToolStatus::Backgrounded);
        m.shell_id = Some(e.shell_id.clone());
        m.is_background = Some(true);
    }
}
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/handlers/tool.ts.
pub(super) fn progress(state: &mut SessionState, e: &g::SessionTaskProgressEvent) {
    if let Some(i) = tool_index(state, &e.tool_use_id) {
        state.session.messages[i].elapsed_seconds = Some(e.elapsed_seconds);
    }
}
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/handlers/tool.ts.
pub(super) fn completed(state: &mut SessionState, e: &g::SessionTaskCompletedEvent) {
    if let Some(m) = state
        .session
        .messages
        .iter_mut()
        .find(|m| m.task_id.as_deref() == Some(&e.task_id))
    {
        m.tool_status = Some(if e.status.to_string() == "failed" {
            g::ToolStatus::Error
        } else {
            g::ToolStatus::Completed
        });
        m.tool_result = Some(
            e.summary
                .as_ref()
                .filter(|s| !s.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("Background task {}", e.status)),
        );
    }
}
