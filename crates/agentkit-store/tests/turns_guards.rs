#![allow(clippy::unwrap_used)]
use agentkit_store::core::turns::{Turn, format_message_timestamp};
use serde_json::json;
#[test]
fn timestamp_projection_uses_explicit_historical_offsets_and_crosses_dates() {
    // 2026-01-02T03:04:05Z: caller resolves offset at this timestamp, not today's offset.
    let timestamp = 1767323045000;
    assert_eq!(
        format_message_timestamp(timestamp, 0).as_deref(),
        Some("2026-01-02 03:04:05")
    );
    assert_eq!(
        format_message_timestamp(timestamp, 8 * 3600).as_deref(),
        Some("2026-01-02 11:04:05")
    );
    assert_eq!(
        format_message_timestamp(timestamp, -5 * 3600).as_deref(),
        Some("2026-01-01 22:04:05")
    );
    assert_eq!(
        format_message_timestamp(timestamp, -4 * 3600).as_deref(),
        Some("2026-01-01 23:04:05")
    );
}
#[test]
fn invalid_timestamp_ranges_or_offsets_return_none() {
    assert!(format_message_timestamp(0, 86400).is_none());
    assert!(format_message_timestamp(i64::MAX, 0).is_none());
    assert!(format_message_timestamp(i64::MIN, 0).is_none());
    // Chrono's last representable UTC date cannot be projected beyond its calendar range.
    assert!(format_message_timestamp(8210266876799999, 3600).is_none());
}
#[test]
fn fixed_card_discriminators_prevent_system_user_and_assistant_confusion() {
    let message = json!({"id":"m","role":"info","content":"notice","timestamp":1});
    let system: Turn =
        serde_json::from_value(json!({"type":"system","message":message,"timestamp":1})).unwrap();
    assert!(matches!(system, Turn::System(_)));
    assert!(
        serde_json::from_value::<Turn>(json!({"type":"future","message":message,"timestamp":1}))
            .is_err()
    );
}
