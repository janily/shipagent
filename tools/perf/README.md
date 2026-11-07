# Native performance suite (T-1-23)

Build and run against the same fixed backend used by Workbench:

```sh
cargo xtask backend build
cargo build --release -p workbench --features perf --locked
cargo build --release -p agentkit-ui --features samples --example performance --locked
node tools/perf/suite.mjs --binary target/release/workbench --backend target/backend/linux-x64 --scenes target/release/examples/performance --output target/perf/report.json --runs 3 --tokens 600 --machine linux-reference --environment 'describe CPU/GPU/display here'
node tools/perf/compare.mjs target/perf/report.json - target/perf/comparison.json
```

Requires Node 22, Python 3 and either a working Linux X11/Wayland display with a Vulkan renderer, or native macOS. On macOS use `target/backend/darwin-arm64` (or `darwin-x64`). The runner starts an in-process loopback model, creates a private temporary fixture/config directory, and launches the real native app. The app configures that local endpoint and sends through its existing controller, authenticated WebSocket, bundled backend and Pi. A real Read result is required before the paced final Markdown. No synthetic chat/tool events are injected into the view.

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

The manual `Linux software-renderer performance experiment` workflow builds release and uploads reports from shared Ubuntu + Xvfb/lavapipe. It is an experiment, not the reference-hardware perf gate. The `Native performance baselines` workflow now provides nightly, manual and related-PR suites on shared Linux and macOS; the older manual stream-only experiment remains available.

## Native scenes and aggregation

The suite runs the real Agent stream three times, then runs a separate native GPUI executable three times. Scene data is explicitly synthetic and never presented as a real Agent conversation. It renders the production WorkbenchView/SessionStore, ToolCard, ToolOutputView and FullToolEditor:

- Frontend RSS after five empty frames, and after loading/retaining ten sessions of 100 messages each. Linux reads VmRSS; macOS reads its own RSS with ps. The separate backend is excluded; shared libraries/renderer allocations in the scene process are included. The scene does not contain the full app controller/RPC/preferences/settings entities, so these snapshots do not establish production-process idle/ten-session RSS acceptance or a long-term memory plateau.
- Open 10,000 alternating Markdown messages, including fixture construction, indexing, binding and the first native submission. No disk/network history download is timed.
- Scroll to 120 deterministic positions, collecting actual Draw/Present work and submission cadence. This is programmatic scroll throughput, not physical touchpad input latency. Every Present must be accounted for, with at least 95% measured work coverage.
- A collapsed 100,000-line Read card, its bounded preview (at most 2,000 lines), and the complete read-only Editor (exactly 100,000 lines). Open timings include construction and first submission; full Editor virtualization remains production behavior.

`startup.firstSubmissionMs`, `backendReadyMs` and `fixtureReadyMs` are app-entry-relative observations in a fresh fixture profile. They exclude the dynamic loader and do not establish cold-process-start-to-input-ready: fixture readiness includes endpoint configuration. `batchReceiptToPresent` is a controller-batch metric, not network event latency. Physical display latency, reference hardware budgets, forced-exit macOS orphan ownership and seven-day stability remain unmeasured. Default production builds have no probes.

The schema v2 report records binary SHA256 values and the CI source commit for audit, retains every raw run and takes the median of each metric (including each run's p95); invalid/missing samples invalidate the aggregate. Scene and stream build optimization, exact workload, cleanup, pacing, text/tool completion and rendering coverage are checked again when comparing artifacts. Local budget misses (16ms work p95, 150/300MB frontend RSS, 800ms history open, 60fps scroll) are reported separately from hardware acceptance. Slow valid runs are not removed.

Fingerprinting covers platform/CPU/GPU/memory/kernel/display backend, cgroup limits, machine label, workload, native viewport and measurement-source hash. It deliberately excludes commit ID so production changes can be compared. A hardware, method or workload change makes the previous report incompatible. Shared hardware load is not controlled by a fingerprint; shared CI is engineering evidence only.

## Nightly comparison and reference runner

The downloader selects the immediately preceding **scheduled** main run by creation time, including failed or unfinished runs; manual/PR runs never replace the previous night. It requests only the same machine's artifact. Missing, invalid or incompatible reports produce explicit warnings; it never searches backwards for a favorable result. It compares at least three complete runs, warning for strictly greater than 10% increases in costs/memory or decreases in FPS. First-run/missing-baseline warnings are expected. Reports, comparison and baseline-selection status are retained as artifacts for 30 days. Exit 2 means invalid data or a local budget miss and is preserved as a CI warning; infrastructure/build failures fail the job.

Shared Linux uses Xvfb/lavapipe; shared macOS uses the runner's virtual display. The optional main-only `reference` job requires both repository variables:

- `MACOS_PERF_RUNNER`: JSON runner labels for an isolated macOS arm64 machine.
- `MACOS_PERF_ISOLATION=disposable-machine-per-job`: an operator assertion that a fresh OS instance is provisioned for each job and destroyed after success, failure **and cancellation**. A persistent self-hosted runner alone does not meet this contract. Configure a main-only protected `performance-reference` environment before enabling it.

Linux retains CHILD_SUBREAPER ownership supervision and reaps descendants. macOS normal exit records the owned sidecar supervisor's shutdown result; this is not an orphan census. On failure the runner terminates, then kills a stuck native process, invalidates cleanup, stops further samples/scenes, and relies on disposal of that job's machine to contain any independent backend process group. Never run this macOS suite on a persistent personal/reference desktop. A configured label does not imply physical M1 acceptance; retain `referenceAcceptance: not_evaluated` until separate documented hardware acceptance.
