# Sidecar supervisor (T-1-05)

`SidecarHandle::start` awaits readiness. A dedicated owner thread serializes child generations, commands, pipe readers and process-tree cleanup. The handle is not Clone; dropping or cancelling initial startup requests cleanup off the calling/GUI thread. `shutdown(grace)` consumes it and cancels retry/backoff; `restart()` obtains a fresh token and readiness endpoint without consuming the crash budget.

Unexpected exits broadcast `Exited`, then restart after one second. The fourth exit in a rolling sixty seconds broadcasts `Fatal(RestartLimit)`. Startup failures and `AlreadyRunning { pid }` are terminal without automatic retry. `events()` gives independent receivers, seeded with the latest event so a fast failure is observable; `last_error()` exposes the latest terminal classification. Endpoint remains the last ready value during recovery; use `Restarted` to replace the RPC endpoint, wait for its handshake and refresh stale data. Never replay old writes.

Unix launches use a new session/process group through process-wrap's safe API. Shutdown sends group SIGTERM, waits grace, then SIGKILLs surviving group members and reaps the leader. Windows uses command-group's suspended launch, attaches the child to a kill-on-close Job Object, then resumes it. Windows shutdown terminates the job immediately (there is no Unix SIGTERM protocol), and OS handle closure also kills members if the supervising application dies. No application unsafe code or GPUI dependency is added. Unix descendants deliberately leaving the owned process group are outside this ownership boundary; pipe joins are bounded even if an escaped process retains them.

Protected launch variables (profile, random token, local host/port, packaged asset paths) override `extra_env`; WebUI, health-port and all RPC TLS overrides are removed. Resource/bin directories precede the user's PATH. Linux/Windows inherit the parent environment. macOS imports a cached login-shell environment using a five-second, 1MiB capture limit, omits VITE variables, and falls back to inherited values. Shutdown cancels an in-progress shell query. The implementation never mutates the parent process environment.

Logs default to `CONFIG_DIR/logs/sidecar.log`; the macOS application uses `~/Library/Logs/<bundle_id>`. Files rotate at 2MiB with one previous file. Stderr tail is at most64KiB; lines over64KiB are omitted, preventing partial-token leakage at truncation boundaries. Token announcement lines are discarded; current token and secret-bearing extra-env values are redacted on both pipes. Config Debug never prints extra_env. Log creation/rotation uses mode0600 on Unix. Runtime write errors warn without stopping pipe drainage; diagnose filesystem permissions if logs are unavailable.

Real lifecycle tests use the packaged Bun:

```sh
AGENTKIT_BACKEND_DIR="$PWD/target/backend/linux-x64" \
  cargo test -p agentkit-sidecar --test manager --locked -- --ignored \
    --skip owned_supervisor_fixture --skip shell_probe_fixture
```

The CI `contract` job runs this on Linux/macOS/Windows. Five common cases cover crash/token/budget, occupied/timeout/manual restart, environment/broadcast/descendants/backoff shutdown, actual backend lock/recovery/RPC replacement, and cancelled startup. Unix adds a descendant grace-period flush test. macOS adds a real zsh capture plus login-shell import/cache/timeout; Windows adds abrupt-supervisor-death job cleanup. Test-only subprocess entrypoints are skipped in the top-level run.
