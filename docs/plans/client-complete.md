# T-1-02 Client implementation plan

> Use executing-plans inline, TDD, one independent whole-branch review.

Goal: complete local RPC transport lifecycle and protocol handling from specs 02/03; preserve pending-write safety.
Architecture: one smol socket owner, observable shared metadata, per-connection capability tasks, independent endpoint/stop controls. Typed business facade remains T-1-03; native capability implementations T-1-19; remote TLS/custom CA T-4-03.
Spec: docs/spec/02 协议与数据模型.md; docs/spec/03 模块详细设计.md.
Constraints: protocol1.0; loopback ws; no browser capability; 30s requests, 10s handshake; reconnect1s×2^n capped30s, stable10s resets; seq ack5s; no write replay; bytes wrapper __craftRpcType/u8. Preserve token redaction.

## Interfaces and sequence

1. Lifecycle (`lib.rs`, new `state.rs`, `transport.rs`; tests `recovery.rs`): ClientOptions max_reconnect_delay; ConnectionState Reconnecting{attempt,next_retry_ms}/Fatal plus existing Disconnected for explicit close; state_changes, has_channel/server_version; replace_endpoint and switch_workspace; recovery snapshot and __transport:reconnected(stale). Tests first: real TCP reconnect, immediate pending rejection and no write replay, stale/fresh/resumed seq handshake, endpoint reset and metadata replacement, close interrupts wait. Run cargo test -p agentkit-client --locked (RED→GREEN), commit.
2. Dispatch/codec (`capability.rs`, transport; tests `capabilities.rs`): registered handlers available before handshake via options; reverse RPC executes off IO thread, bounded concurrent work/timeouts; replies only to originating connection; unknown/browser unavailable. Native binary invoke and subscription APIs use RpcValue; validate malformed binary wrappers on all wire args/results. Size after JSON UTF8/base64 serialization including capability replies. Test slow handler alongside normal response/event, errors, binary roundtrip, timeout, oversized reply and late handler after reconnect. Run client suite RED→GREEN, commit.
3. Consumer safety and evidence (`api.rs`, app controller monitor, docs/tasks/report): expose transport states/recovery; prototype UI disables sending on loss and requires reload on stale recovery rather than silently continuing stale data; full app automatic reload UX T-1-21. Generic invoke never retries; explicit invoke_read retries only timeout once for callers declaring idempotency. Test ordinary write sent once/read retry, current native/core/real backend contract; full features clippy/fmt/drift; three-platform CI; one final review and important fix pass. Commit and PR, no merge.

Preflight: state generation fences queued writes and endpoint changes; per-connection handler replies cannot cross generation. API uses raw wire Value compatibility; RpcValue APIs expose binary leaves explicitly. App prototype full recovery UX is not implemented here.
Review focus: socket loss while enqueueing writes; repeated failed handshakes preserving old reconnect cursor; endpoint swap during backoff/handler; slow or panicking capability; observer lifetime and stale app interaction. Cover each in the corresponding integration test.
