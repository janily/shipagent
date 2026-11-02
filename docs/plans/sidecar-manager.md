# T-1-05 sidecar manager

Base main208884a, PR11 merged. Authority: spec03 sidecar lifecycle, spec07 environment, F01. Implement inline in three slices, with standard/spec review before final commits.

1. Process/session ownership, safe output/logging and environment: immutable protected launch environment; 32-byte fresh tokens each generation; bounded/redacted stdout/stderr log and 64KiB stderr tail; macOS cached login-shell environment with five-second deadline. Unix setsid via safe process-wrap API; Windows suspended launch + kill-on-close Job Object via safe command-group API. No unsafe code in application crates.
2. Single background owner actor: startup/monitor/manual restart/shutdown, broadcast Exited/Restarted/Fatal events, one-second restart delay, fourth crash in rolling60s fatal, AlreadyRunning PID no retry, cancellation/drop cleanup and no restart after shutdown. App routes new endpoints into existing RPC client; stale cache remains disabled until reload (T1-08/21).
3. Real Bun process fixtures + actual backend lifecycle: crash recovery, fresh token/port + RPC endpoint replacement, lock conflict, repeated crash budget, explicit restart, output flood/redaction/timeout, descendant cleanup (inclWindows supervisor abrupt death). Three-platform contract CI includes lifecycle tests; core/native/clippy/fmt/drift, docs/evidence, commit/push/PR without merge.

Scope: no single-instance window focusing (F03), release signing, full recovery UX/snapshot reconciliation, or process descendants that deliberately escape their Unix process group. Windows uses a dependency's safe job API; platform tests must verify attachment and parent-death cleanup.
