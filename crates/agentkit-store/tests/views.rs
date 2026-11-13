#![allow(clippy::unwrap_used)]
use agentkit_protocol::generated as g;
use agentkit_store::core::views::{View, counts, indices};
use std::collections::HashSet;
fn session(id: &str, value: serde_json::Value) -> g::Session {
    let mut v = serde_json::json!({"id":id,"workspaceId":"w","workspaceName":"W","messages":[],"isProcessing":false,"lastMessageAt":1});
    v.as_object_mut()
        .unwrap()
        .extend(value.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}
#[test]
fn six_views_keep_workspace_hidden_archive_counts_and_recent_order_consistent() {
    let s = vec![
        session("new", serde_json::json!({"lastMessageAt":20})),
        session(
            "old-flag",
            serde_json::json!({"isFlagged":true,"lastMessageAt":1,"hasUnread":true}),
        ),
        session("run", serde_json::json!({"isProcessing":true})),
        session("plan", serde_json::json!({"lastMessageRole":"plan"})),
        session(
            "archived",
            serde_json::json!({"isArchived":true,"hasUnread":true,"isFlagged":true}),
        ),
        session(
            "hidden",
            serde_json::json!({"hidden":true,"hasUnread":true}),
        ),
        session("other", serde_json::json!({"workspaceId":"other"})),
    ];
    let plans = HashSet::from(["plan".into()]);
    assert_eq!(counts(&s, &plans, Some("w")), [4, 1, 1, 1, 1, 1]);
    assert_eq!(
        indices(&s, &plans, Some("w"), View::All, ""),
        vec![0, 1, 3, 2]
    );
    assert_eq!(indices(&s, &plans, Some("w"), View::Archived, ""), vec![4]);
    assert_eq!(indices(&s, &plans, Some("w"), View::Unread, ""), vec![1]);
    assert!(indices(&s, &plans, Some("w"), View::All, "no match").is_empty());
}
#[test]
fn titles_are_unicode_case_insensitive_and_plans_do_not_include_running_sessions() {
    let s = vec![
        session("a", serde_json::json!({"name":"中文 ÉCOLE"})),
        session(
            "b",
            serde_json::json!({"isProcessing":true,"lastMessageRole":"plan"}),
        ),
    ];
    let plans = HashSet::from(["a".into(), "b".into()]);
    assert_eq!(indices(&s, &plans, None, View::All, "école"), vec![0]);
    assert_eq!(indices(&s, &plans, None, View::Plans, ""), vec![0]);
}
#[test]
fn cold_plan_review_survives_assistant_commentary_but_not_the_next_visible_user() {
    use agentkit_store::core::views::pending_review;
    let mut s = session(
        "a",
        serde_json::json!({"messages":[{"id":"u","role":"user","content":"plan","timestamp":1},{"id":"p","role":"plan","content":"/plan.md","timestamp":2},{"id":"a","role":"assistant","content":"Ready","timestamp":3}]}),
    );
    assert!(pending_review(&s));
    s.messages.push(serde_json::from_value(serde_json::json!({"id":"hidden","role":"user","content":"internal","timestamp":4,"hidden":true})).unwrap());
    assert!(pending_review(&s));
    s.messages.last_mut().unwrap().hidden = Some(false);
    assert!(!pending_review(&s));
}
