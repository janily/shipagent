# T-1-04 client contracts

```sh
cargo xtask backend build --contract
AGENTKIT_BACKEND_DIR="$PWD/target/backend/linux-x64" \
AGENTKIT_CONTRACT_SERVER="$PWD/target/contract/linux-x64/transport.js" \
  cargo test -p agentkit-client --test contract --locked -- --ignored
AGENTKIT_BACKEND_DIR="$PWD/target/backend/linux-x64" \
  cargo test -p agentkit-sidecar --test agent_turn --locked -- --ignored
```

Use `darwin-arm64` or `win32-x64` for those hosts (PowerShell: set the two `$env:` variables). Requires the pinned submodule, Bun >=1.3.14, Node >=22 and the backend build prerequisites in docs/development.md. `--contract` builds the normal backend and a separate test server in the same disposable pinned checkout with the same lockfile; ordinary builds omit it. The test server is never copied into `target/backend` or release packages.

CI `contract` explicitly runs these ignored tests on Linux, macOS and Windows. Missing environment variables, artifacts, or readiness fail the explicit run; default `cargo test` skips it deliberately. Each process has an isolated profile, generated temporary token, cleanup on unwind and bounded waits. A nonignored helper regression checks that a deliberately incomplete WebSocket upgrade cannot block proxy cleanup.

`contract.rs` has three cases:

- **Actual bundled sidecar:** health, workspace binding, session create/list/query/rename/delete, typed name-change event, draft set/get/delete, stored attachment content, production binary read, production file-picker reverse callback success and failure. The callback is an in-memory test capability, not a native OS dialog.
- **Pinned upstream WsRpcServer transport:** auth rejection/Fatal, handshake metadata/channel/limit, nested and empty Uint8Array roundtrips, reverse success/error/missing/filtered browser/timeout, slow handler concurrency, request deadline/late response correlation, and oversized outbound frame rejection.
- **Real replay and stale:** frame-transparent TCP/WebSocket bridge drops sockets and pauses next accept. A separate client pushes while disconnected. Normal replay retains identity/order/cursor; more than the upstream exported buffer limit forces stale and refresh watermark. A gated committed write fails on disconnect, is not replayed, and old bound handles cannot issue it after recovery. No private server state or forged handshake/seq/stale frames.

Only the last two cases use `transport.ts` test handlers, on the unmodified production server/codec class. They establish wire compatibility; they do not assert production registration of synthetic `contract:*` channels. The existing `agent_turn` case separately verifies actual scripted model streaming, Read tool, final persistence and cancellation, with no external model credentials.

T-1-04 does not implement native capability bodies, reconnect UI/reconciliation, U1–U6 complete application journeys, fixture recording for every stage-one scenario, remote TLS, physical desktop/performance or signed release acceptance.
