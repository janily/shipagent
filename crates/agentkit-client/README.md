# Local RPC client (T-1-02 / T-1-03)

One `agentkit-rpc` thread owns the loopback WebSocket. `connect` returns immediately; ordinary `invoke` queues until handshake, uses a 30s deadline and never retries. Typed business methods are grouped under `api`. Remote wss/custom CA is T-4-03.

- `state`, `state_changes`, `snapshot`, `has_channel`, `server_version` expose lifecycle/handshake metadata. State observers begin with the current state. Disconnected means explicit closure; authentication/protocol/configuration errors enter Fatal and wait for endpoint replacement. Spawning/Restarting are reserved for the sidecar owner.
- Reconnect delays are 1/2/4/8/16/30 seconds, capped by max_reconnect_delay. Only a connection stable for 10s resets the count. Failed handshakes preserve the last good clientId/maximum seq. Seq gaps warn without logging payloads; delivery order is preserved. Sequence ack every 5s has a nonempty id; ping/pong is handled by the WebSocket library.
- `bind_connection` atomically captures a connected handle and metadata before deferring a user intent. Bound RPCs check the captured generation and Connected state under the enqueue lock, so even a future first polled after recovery cannot migrate the old action. Ordinary root handles retain the queue-during-connect behavior.
- Socket loss invalidates queued/sent calls in that generation with CLIENT_DISCONNECTED. Request UUIDs are never reused or replayed. New calls during backoff may queue for the next connection, bounded by their own deadlines. A successful workspace switch updates future handshake binding before its response reaches the caller.
- Snapshot `recovery_id` increments on successful handshake. `needs_refresh` describes that handshake; `refresh_epoch` retains the last generation requiring reload, including across later resumed connections. Consumers compare it to the generation of their loaded snapshot, so a missed polling sample cannot erase stale debt. Existing subscribers receive `__transport:reconnected` with one bool argument (true = reload). This is a refresh notification, not a cache reload or snapshot/event reconciliation implementation.
- `replace_endpoint` interrupts backoff/connect/socket work, rejects the old generation and clears its cursor/metadata. Replacement can recover Fatal. `close` stops all retries; dropping every client clone also ends the worker. Existing subscriptions survive reconnect but finish after explicit close.
- `invoke_read` is an explicit caller promise of idempotency: it retries CLIENT_REQUEST_TIMEOUT/REQUEST_TIMEOUT once, using a new id. Never use it for writes. Generic invoke/invoke_timeout do not opt into automatic read retries; the typed API selects this only for reviewed reads.
- `RpcError::protocol_code` provides wire-compatible codes without parsing error text. TooLarge remains a local enum variant with TRANSFER_TOO_LARGE; Timeout maps CLIENT_REQUEST_TIMEOUT and Disconnected maps CLIENT_DISCONNECTED. Authentication/protocol Wire errors retain their codes.

## Reverse RPC and binary values

`ClientOptions.capabilities` maps implemented channel names to `CapabilityHandler` before connecting. Handlers receive `Vec<RpcValue>` and return an async `Result<RpcValue, WireError>`; Bytes are recursively decoded/encoded. Defaults advertise nothing. Browser invoke is always filtered. `handle_capability` replaces handlers; new names are advertised only on the next handshake.

Each connection owns at most 64 concurrent handler tasks, running outside the socket thread. Unknown/busy handlers return CAPABILITY_UNAVAILABLE; timeout (default30s) returns CLIENT_REQUEST_TIMEOUT; caught panics return HANDLER_ERROR. Replies preserve id/channel. Disconnect cancels awaiting handler futures and drops their reply queue, preventing a late reply crossing sockets. A native action already performed cannot be undone by dropping its RPC future; actual GPUI/native capability bodies and their cancellation/consent belong to T-1-19. Futures must not block and should forward native work to the foreground thread. Panic hooks belong to the caller/runtime; the client does not replace the process panic hook.

`invoke_values` / `subscribe_values` expose native binary leaves. Raw Value APIs retain JSON wrappers for compatibility, but wire args/results are recursively validated so malformed base64 cannot escape by choosing Value. `session_events` returns Result<DecodedSessionEvent, RpcError>, surfacing malformed known events rather than silently dropping them; future tags preserve raw fields. Dropping a mapped subscription releases its forwarding task.

All outbound business frames and capability replies are limited by serialized UTF8 JSON bytes, including base64 expansion. Oversized reverse results become correlated TRANSFER_TOO_LARGE errors; if even that error cannot fit, the connection fails. Legacy servers without maxMessageBytes use the generated 16MiB fallback. Incoming WebSocket messages/frames are independently capped at 16MiB. No compression or remote TLS is introduced.

## Prototype application integration

The current workbench disables actions on loss. Fully resumed history can restore availability. A fresh/stale generation remains disabled until manual reload, even after later nonstale resumes. A persistent refresh watermark handles missed status samples; a bound call prevents writes across the gap between intent acceptance and deferred enqueue. Draft/history are retained. Full automatic cache refresh, snapshot/event reconciliation and recovery UX remain T-1-08/T-1-21; this is not phase-zero acceptance.

## Typed business API

Use `client.sessions()`, `workspaces()`, `drafts()`, `files()`, `skills()`, `connections()`, `oauth()`, `settings()`, `service()` and `events()`. The 79 operations have fixed per-attempt budgets and read/write policy in `V1_OPERATIONS`; reads retry timeout once, mutations never retry. Send optional suffixes preserve positional arrays, model clearing is explicit null, command replies are discriminated and unsuccessful action outcomes become HANDLER_ERROR. Eight typed event streams surface malformed payloads as errors; future session tags remain preserved.

Secret-bearing requests/responses live in `api::secrets` with SecretString and redacted Debug. Generated types cover safe wire DTOs; four sensitive schemas are drift-audited without generating plaintext secret fields. OAuth methods only implement protocol calls; native consent/browser integration is separate. Pending P01/P02 channels remain unavailable until backend registration.

WorkbenchApi delegates to these groups and validates generated DTOs before projecting into the temporary prototype UI model. See [verification](../../docs/verification-typed-api-2026-11-02.md) for coverage and upstream contract differences.
