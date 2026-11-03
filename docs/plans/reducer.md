# T-1-06 reducer port

Base main37f9636, PR12 merged. Authority spec03 core::reducer and spec06 T1-06; actual pinned upstream renderer processor/handlers/helpers determine semantics. No GPUI/network dependency in pure core.

1. Pin/capture upstream8 test files and executable oracle. Run original assertions, capture normalized wire-valid input/output for every handler/processor call, with fixed clock and recorded generated IDs; provenance/drift check in CI. Required Session/Message wire fields absent only in upstream `as any` fixtures get explicit test-only defaults, no production serde weakening.
2. Public core::reducer uses generated Session/Message/event types, typed Streaming/Effect and explicit deterministic Context(now_ms,next_message_id); Result reports narrow generated nested-DTO structural conversion failure, no blanket JSON state. Port text/retry/tools/background/lifecycle/metadata/annotations/usage and foreign/unknown noops. Port eight source suites as Rust modules against executable oracle, add focused branch/error/effect tests.
3. Exercise new reducer in real Agent contract alongside existing prototype, without claiming Entity/UI migration (T1-08). CI oracle drift + existing three-platform core/native/contracts. Standards/spec review, meaningful regression fixes, commits/push/PR without merge.

Decisions: context is explicit to remove upstream Date.now/global-ID counter impurity. API borrows immutable state and returns a new typed state/result. Existing ChatState remains a compatibility projection until T1-08; full turn grouping T1-07. Future event tags retained by DecodedSessionEvent and ignored by reducer, not dropped at transport.
