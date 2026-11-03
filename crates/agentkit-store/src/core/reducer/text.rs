//! Ported from mkagent v0.2.6 event-processor/handlers/text.ts.
use super::*;
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/handlers/text.ts.
pub(super) fn delta(state: &mut SessionState, e: &g::SessionTextDeltaEvent, ctx: &Context) {
    let index = streaming(&state.session.messages, e.turn_id.as_deref());
    delta_at(state, e, ctx, index);
}
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/handlers/text.ts.
/// Indexed variant used by the session cache.
pub(crate) fn delta_at(
    state: &mut SessionState,
    e: &g::SessionTextDeltaEvent,
    ctx: &Context,
    index: Option<usize>,
) {
    if let Some(stream) = &mut state.streaming {
        stream.content.push_str(&e.delta);
        if e.turn_id.is_some() {
            stream.turn_id = e.turn_id.clone();
        }
    } else {
        state.streaming = Some(Streaming {
            content: e.delta.clone(),
            turn_id: e.turn_id.clone(),
            parent_tool_use_id: None,
        });
    }
    if let Some(i) = index {
        state.session.messages[i].content.push_str(&e.delta);
    } else {
        let mut m = message(ctx, g::MessageRole::Assistant, e.delta.clone());
        m.turn_id = e.turn_id.clone();
        m.is_streaming = Some(true);
        m.is_pending = Some(true);
        append(&mut state.session, m, false, ctx);
    }
}
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/handlers/text.ts.
pub(super) fn complete(
    state: &mut SessionState,
    e: &g::SessionTextCompleteEvent,
    ctx: &Context,
) -> Result<(), ReducerError> {
    let index = streaming(&state.session.messages, e.turn_id.as_deref())
        .or_else(|| assistant(&state.session.messages, e.turn_id.as_deref()));
    let index = index.filter(|i| {
        let m = &state.session.messages[*i];
        !(m.is_streaming != Some(true)
            && m.is_intermediate == Some(true)
            && e.is_intermediate == Some(true))
    });
    let text = if !e.text.is_empty() {
        e.text.clone()
    } else {
        state
            .streaming
            .as_ref()
            .map(|s| s.content.clone())
            .filter(|s| !s.is_empty())
            .or_else(|| index.map(|i| state.session.messages[i].content.clone()))
            .unwrap_or_default()
    };
    if let Some(i) = index {
        let m = &mut state.session.messages[i];
        m.content = text;
        if let Some(id) = e.message_id.as_ref().filter(|id| !id.is_empty()) {
            m.id = id.clone();
        }
        if let Some(t) = e.timestamp.filter(|t| *t != 0.0) {
            m.timestamp = time(Some(t), ctx.now_ms)?;
        }
        m.is_streaming = Some(false);
        m.is_pending = Some(false);
        m.is_intermediate = e.is_intermediate;
        m.turn_id = e.turn_id.clone();
        m.parent_tool_use_id = e.parent_tool_use_id.clone();
        if e.is_intermediate != Some(true) {
            state.session.last_message_at = ctx.now_ms;
        }
    } else {
        let mut m = message(ctx, g::MessageRole::Assistant, text);
        if let Some(id) = &e.message_id {
            m.id = id.clone();
        }
        m.timestamp = time(e.timestamp, ctx.now_ms)?;
        m.is_streaming = Some(false);
        m.is_pending = Some(false);
        m.is_intermediate = e.is_intermediate;
        m.turn_id = e.turn_id.clone();
        m.parent_tool_use_id = e.parent_tool_use_id.clone();
        append(&mut state.session, m, e.is_intermediate != Some(true), ctx);
    }
    state.streaming = None;
    Ok(())
}
/// Ported from mkagent v0.2.6 apps/electron/src/renderer/event-processor/handlers/text.ts.
pub(super) fn discard(state: &mut SessionState, e: &g::SessionTextDiscardEvent) {
    state.session.messages.retain(|m| {
        !(m.role == g::MessageRole::Assistant
            && m.turn_id.as_deref() == Some(&e.turn_id)
            && (m.is_streaming == Some(true) || m.is_pending == Some(true)))
    });
    if state
        .streaming
        .as_ref()
        .is_some_and(|s| s.turn_id.as_deref() == Some(&e.turn_id))
    {
        state.streaming = None;
    }
}
