#![allow(clippy::unwrap_used)]
use agentkit_protocol::generated as g;
use agentkit_store::core::session::SessionModel;
#[test]
fn cache_opens_a_formal_session() {
    let session:g::Session=serde_json::from_value(serde_json::json!({"id":"s","workspaceId":"w","workspaceName":"W","messages":[],"lastMessageAt":0,"isProcessing":false})).unwrap();
    let model = SessionModel::new(session);
    assert!(model.turns.is_empty());
}
fn snapshot() -> g::Session {
    serde_json::from_value(serde_json::json!({"id":"s","workspaceId":"w","workspaceName":"W","messages":[],"lastMessageAt":0,"isProcessing":false})).unwrap()
}
fn ctx(id: &str) -> agentkit_store::core::reducer::Context {
    agentkit_store::core::reducer::Context {
        now_ms: 1700000000000,
        message_id: id.into(),
    }
}
fn event(v: serde_json::Value) -> agentkit_protocol::DecodedSessionEvent {
    serde_json::from_value(v).unwrap()
}
fn equal_turns(model: &SessionModel) {
    let expected = agentkit_store::core::turns::group_messages_by_turn(
        &model.state.session.messages,
        agentkit_store::core::turns::GroupTurnsOptions {
            is_session_processing: Some(model.state.session.is_processing),
        },
    );
    assert_eq!(
        serde_json::to_value(&model.turns).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}
#[test]
fn every_upstream_reducer_call_has_the_same_cached_turns_as_full_projection() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/upstream-reducer.json")).unwrap();
    for record in fixtures["records"].as_array().unwrap() {
        let input: agentkit_store::core::reducer::SessionState =
            serde_json::from_value(record["input"].clone()).unwrap();
        let mut model = SessionModel::new(input.session);
        model.state.streaming = input.streaming;
        model
            .apply(
                &event(record["event"].clone()),
                &serde_json::from_value(record["context"].clone()).unwrap(),
            )
            .unwrap();
        equal_turns(&model);
    }
}
#[test]
fn thousand_deltas_preserve_utf8_and_do_not_regroup_or_replace_history() {
    let mut model = SessionModel::new(snapshot());
    let count = model.regroup_count;
    for i in 0..1000 {
        model.apply(&event(serde_json::json!({"type":"text_delta","sessionId":"s","delta":"中🚢","turnId":"t"})),&ctx(&i.to_string())).unwrap();
    }
    assert_eq!(model.regroup_count, count);
    assert_eq!(model.delta_count, 1000);
    assert_eq!(model.state.session.messages.len(), 1);
    assert_eq!(model.state.session.messages[0].content, "中🚢".repeat(1000));
    equal_turns(&model);
    model.apply(&event(serde_json::json!({"type":"text_complete","sessionId":"s","text":"corrected","messageId":"canonical","turnId":"t","timestamp":1700000000001_i64})),&ctx("end")).unwrap();
    equal_turns(&model);
    assert_eq!(model.state.session.messages[0].id, "canonical");
}
#[test]
fn optimistic_sends_queue_restore_and_preserve_later_edits() {
    let mut model = SessionModel::new(snapshot());
    model.draft.text = "one".into();
    model.begin_send("one".into(), vec![], &ctx("u1")).unwrap();
    model.begin_send("two".into(), vec![], &ctx("u2")).unwrap();
    assert_eq!(model.state.session.messages[1].is_queued, Some(true));
    model.draft.text = "new edit".into();
    model.finish_send("u2", false);
    assert_eq!(model.draft.text, "new edit");
    assert!(model.state.session.is_processing);
    model.finish_send("u1", true);
    assert_eq!(model.draft.text, "new edit");
    equal_turns(&model);
}
#[test]
fn canonical_user_event_prevents_lost_ack_from_removing_the_message() {
    let mut model = SessionModel::new(snapshot());
    model.begin_send("send".into(), vec![], &ctx("u")).unwrap();
    model.apply(&event(serde_json::json!({"type":"user_message","sessionId":"s","status":"accepted","optimisticMessageId":"u","message":{"id":"canonical","role":"user","content":"send","timestamp":1700000000000_i64}})),&ctx("e")).unwrap();
    model.finish_send("u", false);
    assert_eq!(model.state.session.messages.len(), 1);
    equal_turns(&model);
}
#[test]
fn unopened_sessions_receive_processing_permissions_metadata_and_deletion() {
    use agentkit_store::core::session::SessionSummaries;
    let mut list = SessionSummaries {
        sessions: vec![snapshot()],
        ..Default::default()
    };
    list.apply(&event(
        serde_json::json!({"type":"text_delta","sessionId":"s","delta":"x"}),
    ));
    assert!(list.sessions[0].is_processing);
    list.apply(&event(serde_json::json!({"type":"permission_request","sessionId":"s","request":{"sessionId":"s","requestId":"r","description":"run","toolName":"Bash"}})));
    assert_eq!(list.awaiting["s"], ["r"]);
    list.permission_responded("s", "r");
    assert!(!list.awaiting.contains_key("s"));
    list.apply(&event(
        serde_json::json!({"type":"title_generated","sessionId":"s","title":"renamed"}),
    ));
    assert_eq!(list.sessions[0].name.as_deref(), Some("renamed"));
    list.apply(&event(
        serde_json::json!({"type":"complete","sessionId":"s"}),
    ));
    assert!(!list.sessions[0].is_processing);
    list.apply(&event(
        serde_json::json!({"type":"session_deleted","sessionId":"s"}),
    ));
    assert!(list.sessions.is_empty());
    list.apply(&event(
        serde_json::json!({"type":"session_created","sessionId":"new"}),
    ));
    assert!(list.refresh_required);
}
#[test]
fn late_structural_changes_keep_the_completed_prefix_and_rebuild_only_a_suffix() {
    let mut session = snapshot();
    for i in 0..1000 {
        session.messages.push(serde_json::from_value(serde_json::json!({"id":format!("old{i}"),"role":"assistant","content":"old","timestamp":i})).unwrap());
    }
    let mut model = SessionModel::new(session);
    let before = if let agentkit_store::core::turns::Turn::Assistant(t) = &model.turns[0] {
        t.response.as_ref().unwrap().text.as_ptr()
    } else {
        panic!()
    };
    model.apply(&event(serde_json::json!({"type":"text_delta","sessionId":"s","turnId":"new","delta":"new"})),&ctx("new")).unwrap();
    let update=model.apply(&event(serde_json::json!({"type":"text_complete","sessionId":"s","turnId":"new","text":"final","messageId":"canonical"})),&ctx("done")).unwrap();
    assert!(update.events.iter().any(|e|matches!(e,agentkit_store::core::session::SessionStoreEvent::TurnsChanged{from_turn} if *from_turn>=999)));
    let after = if let agentkit_store::core::turns::Turn::Assistant(t) = &model.turns[0] {
        t.response.as_ref().unwrap().text.as_ptr()
    } else {
        panic!()
    };
    assert_eq!(before, after);
    equal_turns(&model);
}
#[test]
fn overlapping_failed_optimistic_requests_cannot_leave_false_processing() {
    let mut m = SessionModel::new(snapshot());
    m.begin_send("one".into(), vec![], &ctx("one")).unwrap();
    m.begin_send("two".into(), vec![], &ctx("two")).unwrap();
    m.finish_send("one", false);
    m.finish_send("two", false);
    assert!(!m.busy());
    assert!(m.state.session.messages.is_empty());
}
#[test]
fn late_success_ack_does_not_revive_a_completed_turn() {
    let mut m = SessionModel::new(snapshot());
    m.begin_send("one".into(), vec![], &ctx("one")).unwrap();
    m.apply(
        &event(serde_json::json!({"type":"complete","sessionId":"s"})),
        &ctx("done"),
    )
    .unwrap();
    m.finish_send("one", true);
    assert!(!m.busy());
}
#[test]
fn backwards_clock_on_first_delta_uses_a_structural_ordering_fallback() {
    let mut session = snapshot();
    session.messages.push(serde_json::from_value(serde_json::json!({"id":"future","role":"assistant","content":"old","timestamp":1800000000000_i64})).unwrap());
    let mut m = SessionModel::new(session);
    let update = m
        .apply(
            &event(
                serde_json::json!({"type":"text_delta","sessionId":"s","delta":"new","turnId":"t"}),
            ),
            &ctx("new"),
        )
        .unwrap();
    equal_turns(&m);
    assert!(matches!(
        &update.events[0],
        agentkit_store::core::session::SessionStoreEvent::TurnsChanged { .. }
    ));
}
#[test]
fn late_ordered_stream_updates_both_activity_and_promoted_response_without_regroup() {
    let mut session = snapshot();
    session.messages.push(serde_json::from_value(serde_json::json!({"id":"future-user","role":"user","content":"question","timestamp":1800000000000_i64})).unwrap());
    let mut m = SessionModel::new(session);
    for delta in ["A", "B", "C"] {
        m.apply(
            &event(
                serde_json::json!({"type":"text_delta","sessionId":"s","delta":delta,"turnId":"t"}),
            ),
            &ctx("new"),
        )
        .unwrap();
        equal_turns(&m);
    }
}
#[test]
fn invalid_context_foreign_and_unknown_events_leave_the_cache_unchanged() {
    let mut m = SessionModel::new(snapshot());
    assert!(
        !m.apply(
            &event(serde_json::json!({"type":"future","sessionId":"s"})),
            &ctx("x")
        )
        .unwrap()
        .changed
    );
    assert!(
        !m.apply(
            &event(serde_json::json!({"type":"text_delta","sessionId":"other","delta":"bad"})),
            &ctx("x")
        )
        .unwrap()
        .changed
    );
    assert!(
        m.apply(
            &event(serde_json::json!({"type":"text_delta","sessionId":"s","delta":"bad"})),
            &ctx("")
        )
        .is_err()
    );
    assert!(m.state.session.messages.is_empty());
    assert!(m.begin_send(" ".into(), vec![], &ctx("u")).is_none());
    assert!(m.begin_send("a".into(), vec![], &ctx("")).is_none());
    m.begin_send("valid".into(), vec![], &ctx("u")).unwrap();
    assert!(
        m.begin_send("duplicate".into(), vec![], &ctx("u"))
            .is_none()
    );
    assert!(m.finish_send("missing", false).is_none());
}
#[test]
fn snapshot_promoted_intermediate_is_unpromoted_when_streaming_resumes() {
    let mut session = snapshot();
    session.messages.push(serde_json::from_value(serde_json::json!({"id":"partial","role":"assistant","content":"old","timestamp":1,"isStreaming":true,"isPending":true,"turnId":"t"})).unwrap());
    let mut m = SessionModel::new(session);
    m.apply(
        &event(
            serde_json::json!({"type":"text_delta","sessionId":"s","turnId":"t","delta":" new"}),
        ),
        &ctx("unused"),
    )
    .unwrap();
    equal_turns(&m);
}
#[test]
fn hidden_streams_remain_absent_from_visible_cards() {
    let mut session = snapshot();
    session.messages.push(serde_json::from_value(serde_json::json!({"id":"hidden","role":"assistant","content":"internal","timestamp":1,"isStreaming":true,"isPending":true,"hidden":true})).unwrap());
    let mut m = SessionModel::new(session);
    m.apply(
        &event(serde_json::json!({"type":"text_delta","sessionId":"s","delta":"more"})),
        &ctx("unused"),
    )
    .unwrap();
    equal_turns(&m);
    assert!(m.turns.is_empty());
}
#[test]
fn attachments_are_preserved_by_optimistic_rollback_and_success_consumes_unchanged_text() {
    let attachment:g::StoredAttachment=serde_json::from_value(serde_json::json!({"id":"a","name":"proof.txt","mimeType":"text/plain","size":1,"storedPath":"/fixture/proof.txt","type":"text"})).unwrap();
    let mut m = SessionModel::new(snapshot());
    let p = m
        .begin_send("".into(), vec![attachment], &ctx("with-file"))
        .unwrap();
    assert_eq!(
        m.state.session.messages[0].attachments.as_ref().unwrap()[0].id,
        "a"
    );
    m.finish_send(&p.id, false);
    assert_eq!(m.draft_attachments[0].id, "a");
    m.draft.text = "new".into();
    m.set_draft_attachments(Vec::new());
    m.begin_send("new".into(), vec![], &ctx("text")).unwrap();
    m.finish_send("text", true);
    assert!(m.draft.text.is_empty());
}
#[test]
fn permissions_and_all_summary_badges_are_session_scoped() {
    use agentkit_store::core::session::SessionSummaries;
    let mut m = SessionModel::new(snapshot());
    let permission = event(
        serde_json::json!({"type":"permission_request","sessionId":"s","request":{"sessionId":"s","requestId":"r","description":"run","toolName":"Bash"}}),
    );
    m.apply(&permission, &ctx("p")).unwrap();
    m.permission_responded("r");
    assert!(m.permissions.is_empty());
    let mut list = SessionSummaries {
        sessions: vec![snapshot()],
        ..Default::default()
    };
    assert!(!list.apply(&event(
        serde_json::json!({"type":"session_created","sessionId":"s"})
    )));
    for value in [
        serde_json::json!({"type":"name_changed","sessionId":"s","name":"name"}),
        serde_json::json!({"type":"session_flagged","sessionId":"s"}),
        serde_json::json!({"type":"session_unflagged","sessionId":"s"}),
        serde_json::json!({"type":"session_archived","sessionId":"s"}),
        serde_json::json!({"type":"session_unarchived","sessionId":"s"}),
        serde_json::json!({"type":"plan_submitted","sessionId":"s","message":{"id":"plan","role":"plan","content":"plan","timestamp":1}}),
    ] {
        assert!(list.apply(&event(value)));
    }
    assert!(list.plans.contains("s"));
    assert_eq!(list.sessions[0].name.as_deref(), Some("name"));
    assert_eq!(list.sessions[0].is_flagged, Some(false));
    assert_eq!(list.sessions[0].is_archived, Some(false));
    assert!(!list.apply(&event(serde_json::json!({"type":"future","sessionId":"s"}))));
    assert!(!list.apply(&event(
        serde_json::json!({"type":"status","sessionId":"s","message":"working"})
    )));
    list.apply(&event(
        serde_json::json!({"type":"name_changed","sessionId":"unknown","name":"new"}),
    ));
    assert!(list.refresh_required);
    list.permission_responded("missing", "missing");
}
#[test]
fn stopping_a_queued_send_preserves_restored_text_against_a_late_ack() {
    let mut session = snapshot();
    session.is_processing = true;
    let mut m = SessionModel::new(session);
    m.set_draft("queued".into());
    m.begin_send("queued".into(), vec![], &ctx("u")).unwrap();
    assert!(m.draft_is_current("u"));
    m.apply(&event(serde_json::json!({"type":"interrupted","sessionId":"s","message":{"id":"stop","role":"info","content":"stopped","timestamp":1700000000001_i64},"queuedMessages":["queued"]})),&ctx("e")).unwrap();
    assert!(!m.draft_is_current("u"));
    m.finish_send("u", true);
    assert_eq!(m.draft.text, "queued");
    assert!(!m.busy());
}
#[test]
fn failed_queued_request_after_interruption_does_not_duplicate_or_drop_restored_input() {
    let mut session = snapshot();
    session.is_processing = true;
    let mut m = SessionModel::new(session);
    m.begin_send("queued".into(), vec![], &ctx("u")).unwrap();
    m.apply(&event(serde_json::json!({"type":"interrupted","sessionId":"s","message":{"id":"stop","role":"info","content":"stopped","timestamp":1700000000001_i64},"queuedMessages":["queued"]})),&ctx("e")).unwrap();
    m.finish_send("u", false);
    assert_eq!(m.draft.text, "queued");
    assert_eq!(m.state.session.messages.len(), 1);
    assert!(!m.busy());
}
#[test]
fn repeated_permissions_are_deduplicated_without_losing_other_requests() {
    let mut m = SessionModel::new(snapshot());
    for id in ["a", "b", "a"] {
        m.apply(&event(serde_json::json!({"type":"permission_request","sessionId":"s","request":{"sessionId":"s","requestId":id,"description":"run","toolName":"Bash"}})),&ctx(id)).unwrap();
    }
    assert_eq!(m.permissions.len(), 2);
    m.permission_responded("a");
    assert_eq!(m.permissions[0].request_id, "b");
}
#[test]
fn text_and_attachment_drafts_restore_together_without_losing_newer_file_edits() {
    let file = |id: &str| {
        serde_json::from_value::<g::StoredAttachment>(serde_json::json!({"id":id,"name":"proof.txt","mimeType":"text/plain","size":1,"storedPath":"/fixture/proof.txt","type":"text"})).unwrap()
    };
    let mut m = SessionModel::new(snapshot());
    m.set_draft("with file".into());
    m.begin_send("with file".into(), vec![file("a")], &ctx("first"))
        .unwrap();
    m.finish_send("first", false);
    assert_eq!(m.draft_attachments[0].id, "a");
    m.begin_send("with file".into(), vec![file("a")], &ctx("retry"))
        .unwrap();
    m.set_draft_attachments(vec![file("b")]);
    assert!(!m.draft_is_current("retry"));
    m.finish_send("retry", true);
    assert_eq!(m.draft.text, "with file");
    assert_eq!(m.draft_attachments[0].id, "b");
    m.begin_send("with file".into(), vec![file("b")], &ctx("last"))
        .unwrap();
    assert!(m.draft_is_current("last"));
    m.finish_send("last", true);
    assert!(m.draft.text.is_empty());
    assert!(m.draft_attachments.is_empty());
}
#[test]
fn visible_message_locations_are_indexed_and_unknown_ids_are_absent() {
    let mut m = SessionModel::new(snapshot());
    assert_eq!(m.turn_index("missing"), None);
    m.apply(
        &event(serde_json::json!({"type":"text_delta","sessionId":"s","delta":"x"})),
        &ctx("stream"),
    )
    .unwrap();
    assert_eq!(m.turn_index("stream"), Some(0));
}

#[test]
fn explicit_resend_never_owns_equal_or_empty_composer_drafts() {
    for accepted in [false, true] {
        for text in ["", "original", "different"] {
            let mut model = SessionModel::new(snapshot());
            model.set_draft(text.into());
            let file:g::StoredAttachment=serde_json::from_value(serde_json::json!({"id":"stored","type":"text","name":"proof.txt","mimeType":"text/plain","size":1,"storedPath":"/tmp/proof.txt"})).unwrap();
            model.set_draft_attachments(vec![file.clone()]);
            let before = model.draft_attachments.clone();
            model
                .begin_resend("original".into(), vec![file], &ctx("retry"))
                .unwrap();
            assert!(!model.draft_is_current("retry"));
            model.finish_send("retry", accepted).unwrap();
            assert_eq!(model.draft.text, text);
            assert_eq!(
                serde_json::to_value(&model.draft_attachments).unwrap(),
                serde_json::to_value(before).unwrap()
            );
        }
    }
}
#[test]
fn workspace_refresh_waits_for_send_ack_even_after_canonical_echo_replaces_pending_message() {
    let mut model = SessionModel::new(snapshot());
    assert!(!model.has_pending_send());
    model.begin_send("draft".into(), vec![], &ctx("u")).unwrap();
    assert!(model.has_pending_send());
    model.apply(&event(serde_json::json!({"type":"user_message","sessionId":"s","status":"accepted","optimisticMessageId":"u","message":{"id":"u","role":"user","content":"draft","timestamp":1700000000000_i64}})),&ctx("echo")).unwrap();
    assert!(model.send_confirmed("u"));
    assert!(model.has_pending_send());
    model.finish_send("u", true);
    assert!(!model.has_pending_send());
}

#[test]
fn plan_snapshot_version_distinguishes_fresh_edits_even_when_text_returns_to_original() {
    let mut model = SessionModel::new(snapshot());
    model.set_draft("plan review context".into());
    let original = model.draft.text.clone();
    let captured = model.draft_revision();
    // An unchanged editor still belongs to its acceptance snapshot.
    model.set_draft(original.clone());
    assert_eq!(model.draft_revision(), captured);
    // Editing away and back creates a new draft with identical text; an old ACK
    // may not consume it just because the final string happens to match again.
    model.set_draft("new follow-up".into());
    model.set_draft(original.clone());
    assert_eq!(model.draft.text, original);
    assert_ne!(model.draft_revision(), captured);
}

#[test]
fn optimistic_mentions_update_cached_turns_without_touching_canonical_messages() {
    use agentkit_store::core::mentions::{message_badges, metadata};
    let mut m = SessionModel::new(snapshot());
    let badges = message_badges(&metadata("[file:src/main.rs]", &[]));
    m.annotate_pending("missing", badges.clone());
    m.begin_send("[file:src/main.rs]".into(), vec![], &ctx("u"))
        .unwrap();
    m.annotate_pending("u", badges.clone());
    assert_eq!(
        m.state.session.messages[0].badges.as_ref().unwrap()[0].label,
        "main.rs"
    );
    equal_turns(&m);
    m.state.session.messages[0].is_pending = None;
    m.annotate_pending("u", None);
    assert!(m.state.session.messages[0].badges.is_some());
}
