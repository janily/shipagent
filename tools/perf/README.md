# Linux native stream experiment

Build and run against the same fixed backend used by Workbench:

```sh
cargo xtask backend build
cargo build --release -p workbench --features perf --locked
node tools/perf/run.mjs --binary target/release/workbench --backend target/backend/linux-x64 --output target/perf/stream.json --runs 3 --tokens 600 --environment 'describe CPU/GPU/display here'
```

Requires Python 3 and a working Linux X11/Wayland display and Vulkan renderer. The runner starts an in-process loopback model, creates a private temporary fixture/config directory, and launches the real native app. The app configures that local endpoint and sends through its existing controller, authenticated WebSocket, bundled backend and Pi. A real Read result is required before the paced final Markdown. No synthetic chat/tool events are injected into the view.

On Xvfb without a window manager, install xdotool and add `--focus xdotool`; desktop compositors normally honor application activation. Focus is checked inside the native window rather than inferred from a focus command exit code.

The output path must be new. Reports contain metrics/counters/environment only. Benchmark mode refuses remote backend attachment and requires the runner's private marker, an empty config directory and isolated working directory. Default builds have no perf feature or automatic benchmark mode. The runner strips inherited API-key/auth-secret variables from the app environment and never records the environment wholesale or conversation text.

## Workload and metric definitions

The simulated model tokenizer defines one token as a whitespace-delimited word with its trailing whitespace. This is explicit, reproducible workload sizing, **not** a vendor BPE token count, a character count or an assumed SSE packet count. The fixture includes headings, bold text, a table and Rust code. One simulated token is emitted every 20ms using monotonic deadlines; token/byte counts, actual emission rate and largest gap are recorded after a complete stream.

Native metrics come from GPUI profiler Draw/Present events for the actual window. Capture begins at the first streamed text event, includes intermediate/tool/final output, and ends 100ms after the terminal event so the completion frame is included. There is no discarded warm-up interval.

- `frameWork`: GPUI main-thread work, from the later of frame signal/first invalidation to platform submission start. Its p95 is compared with 16ms.
- `draw`: Window::draw cost; `submission`: platform submission cost, separately including any blocking in that call.
- `dirtyToPresent`: invalidation to submission end, including scheduling/presentation waits.
- `animationInterval`: GPUI animation presentation intervals, when available. Token spacing is never reported as frame time.
- `controllerUpdate`: reducer/view update batch cost outside the draw callback.
- `batchReceiptToPresent`: first controller receive for a batch to the presentation of the first subsequent draw starting after that batch was applied. This includes the current coalescing wait and is a batch metric, not network/server-to-display latency.
- Unmeasured/unpresented frames and updates remain explicit counts; empty distributions are null, never zero.

A valid measurement requires an optimized release build, successful Agent terminal/tool evidence, exactly the requested model token count, observed 50/s within 5%, no model gap above 100ms, matching final text bytes and a presented completion frame, at least 100 measured frames, at least 95% measurement coverage, and an active native window for at least 95% of updates. Invalid runs and valid slow runs remain in the report. `frameBudgetMet` means only this run's measured GPUI work p95 met 16ms. It does not mean all latency/refresh/GPU or product requirements passed.

The report always keeps `referenceAcceptance: not_evaluated`: cloud software rendering and shared CI do not establish acceptance on the specified Ubuntu i5-8250U machine. Record actual GPU/display conditions in `--environment`; container CPU/memory limits are included alongside host-visible hardware information.

## Failure experiment and exit status

```sh
node tools/perf/run.mjs --binary target/release/workbench --backend target/backend/linux-x64 --output target/perf/disconnect.json --runs 1 --tokens 600 --scenario disconnect --environment 'same machine'
```

Use `--scenario crash` to kill the native app after two seconds and exercise orphan supervision; that run is intentionally invalid and forced cleanup must be reported.

This disconnects the model after 50 final-stream tokens. Expect exit 2 and an invalid report, even if some frames are fast. The app has a 45-second watchdog and orderly shutdown; a Python supervisor has a 65-second deadline and uses Linux CHILD_SUBREAPER to own and reap orphan descendants. The runner has a 75-second outer deadline and never discovers ownership from a reusable bare PID. Forced cleanup invalidates a run. Temporary model connections and directories are closed/removed in finally paths.

Exit 0: all runs valid and within this local frame-work budget. Exit 2: invalid measurement or measured budget miss. CLI/build/filesystem errors may exit 1. A debug smoke is useful for plumbing but is always rejected as an acceptance measurement.

Deterministic tests:

```sh
node --test tools/perf/*.test.mjs tools/fake-llm/server.test.mjs
python3 -m unittest discover -s tools/perf -p 'test_*.py'
cargo test -p agentkit-ui -p agentkit-app --features agentkit-app/perf --locked
```

The manual `Linux software-renderer performance experiment` workflow builds release and uploads reports from shared Ubuntu + Xvfb/lavapipe. It is an experiment, not the reference-hardware perf gate. Formal nightly baselines/10% regression alerting belong to T-1-23.
