# Real event recording (T-1-09)

Build the pinned backend first (`cargo xtask backend build --contract`), then:

```
cargo xtask fixtures record text
cargo xtask fixtures record all
cargo xtask fixtures replay crates/agentkit-store/tests/fixtures/recorded
cargo test -p agentkit-store --test recorded
```

`AGENTKIT_BACKEND_DIR` selects a built backend; `AGENTKIT_NODE` selects Node. `AGENTKIT_FIXTURE_OUTPUT` selects an output directory (default: the committed store fixtures). Unknown scenario names are rejected. The 14 JSON-syntax YAML scripts in `tools/fake-llm/scenarios` define the initial prompt and local model pacing. Only the JSON subset of YAML is supported, avoiding an extra runtime/parser dependency. The scenario driver controls cancel, permissions, in-turn append and compact through typed API calls. The model serves OpenAI chat completions/SSE; it never fabricates `session:event` frames or executes backend tools.

Each capture starts an isolated temporary config/workspace, real Bun/Pi sidecar and loopback fake model. Subscribe before creation; wait for terminal events, idle authoritative snapshots and drained events (70-second scenario deadline). Shut down the owned sidecar and reap the model on success and failure. Successful permission scenarios also verify the real write's presence/absence. No external model, API key, browser or production workspace is used. The nested scenario uses this pinned backend's actual `spawn_session` twice: three separately persisted sessions with a parent/grandparent relationship; it does not claim legacy SDK Task activity-tree equivalence.

Output per scenario:

- `<name>.jsonl`: raw typed wire events, in receive order.
- `<name>.initial.json`: empty initial session snapshots, including child headers. Child headers are acquired at first observation, with messages cleared before replaying their already subscribed event stream; they are replay seeds, not claimed pre-creation RPC snapshots.
- `<name>.final.json`: authoritative `sessions:getMessages` snapshots after quiescence.
- `<name>.meta.json`: reviewed upstream revision/tag, platform, model/script and built server/Pi SHA-256 hashes, artifact checksums and verified event signatures. Source changes during capture abort publication.

Only temporary-root strings are replaced with `<FIXTURE_ROOT>` (both Windows slash forms); IDs, timestamps, event contents and order otherwise remain. Unverified candidates stay in `.pending` and never count as accepted fixtures. Failures leave the previous accepted bundle intact; mixed writes are detected by checksums. Do not commit candidates.

Replay applies every event to the indexed cache and compares its turns with a full projection after every event. The pinned server persists user/assistant/plan/error messages, while renderer tool/status/info rows are transient; unfinished cancelled deltas never become durable assistant replies. Compare all serialized durable message fields, including attachments, relationships and complete error metadata. Remove only id/timestamp, renderer flags isPending/isStreaming/isQueued, isIntermediate (the pinned server omits it from text_complete events), and client-derived errorActions (not persisted by this server). Sort by the original timestamps before removing them: queued user messages keep their insertion position on disk but receive a later processing timestamp. This explicitly reconciles the persisted insertion order with renderer chronology. The raw final snapshot preserves the omitted identity/time/presentation fields for inspection; no additional durable message fields are excluded. Scenario signatures independently require concurrent tool starts, two child generations, denial errors, interruption, three retry phases, typed error, a persisted plan, successful compaction (not “session too small”), and completed background shell.

Default offline tests verify all14 bundles, checksums, script/upstream provenance, negative/corrupt/missing-event cases and scenario evidence. Three-platform contract CI additionally builds a fresh backend, re-records all14, replays them and uploads the new bundles. Platform-specific error prose and shell paths may differ; CI validates each fresh capture against its own authoritative final snapshot.

For interactive development, set `AGENTKIT_RECORD_EVENTS` to a **new** JSONL path before starting the workbench. Background IO records only subscribed session events and redacts the connection token; existing destinations are never overwritten, and IO failure is reported on developer stderr. One worker remains open across reconnects; runtime feeders register each new token before forwarding events. This is explicit transcript capture and may contain message/tool content; it is not normal logging or telemetry. The full fixture command is the reproducible, checksummed acceptance path. Stage0/performance/physical-desktop/signing gates remain open.
