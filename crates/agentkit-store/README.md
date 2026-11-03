# agentkit-store

`core::reducer` is the formal, generated-DTO session event reducer. `process` and `process_decoded` borrow immutable input and return a new `ProcessResult` with typed state and ordered deferred effects. The explicit `Context` carries time and a reserved message ID, keeping reduction independent of IO, GPUI, clocks and randomness. Unknown decoded event tags and events from other sessions leave state unchanged. Invalid Message timestamp projections and generated nested DTO incompatibilities return `ReducerError`.

See the [upstream oracle](../../tools/reducer/README.md) for provenance, fixture generation and tests. The phase-zero `ChatState` remains only as a compatibility/test surface. The native app now uses the generated session cache and optional entities below. Stage-zero acceptance remains open.

`core::turns` projects generated Messages into typed user/system/assistant cards, derives phases and stable UI keys, groups subagent descendants/continued tasks, and exposes summaries/selectors and formatting. Its [upstream oracle](../../tools/turns/README.md) executes all seven original chat test files and distinguishes React source guards from native behavior. Timestamp formatting takes an explicit historical offset so the core stays pure. Reducer→turns→session-cache is verified against each event in the real Agent contract.


`core::session` owns indexed mutable session caches, cached turns, permissions, draft edit generations and optimistic sends. Existing text deltas mutate their backing message/card in place; structural changes regroup only the affected suffix. Timestamp ordering changes explicitly use the structural fallback. `SessionSummaries` routes named session events into header-only list state.

Enable `entities` for GPUI `AppState`, `WorkspaceStore`, `SessionListStore` and per-open `SessionStore`. The controller drains bounded 16 ms batches and routes all open sessions; entities emit coalesced `StreamDelta` or `TurnsChanged` and notify once per batch. Views subscribe/read, while the controller owns mutation, RPC and deferred effects. Default features remain empty so core tests and coverage need no GPUI.
