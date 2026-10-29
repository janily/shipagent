# Linux native streaming performance plan

> Execute inline using superpowers:executing-plans; task commits and push remain authorized.

Goal: close T-0-05 frame measurement and extend T-0-08 with a reproducible 50 scripted-model-token/s workload on the real Linux native/backend path.
Spec: docs/spec/01 F-15 and nonfunctional table; docs/spec/05; docs/spec/06 T-0-05/T-0-08. macOS and Windows deferred by user.

Architecture: isolated fake-model pacing and fixed Markdown fixture; optional `perf` build enables GPUI profiler frame events in the real controller. Node runner creates fresh config/workdir, runs actual Workbench + bundled backend, validates report/sample/rate/terminal state, and shuts down all owned resources. No transcript or credential output in timing reports.

## Metric and evidence rules
- Use release build for measured baselines. Debug/headless tests never satisfy performance acceptance.
- Measure GPUI FrameTiming.work at platform submission, draw duration and dirty-to-present separately; do not equate 20ms token spacing with frame work.
- Fixture tokenizer is explicitly simulated: whitespace-delimited pieces including trailing whitespace. Record token count, bytes, observed emission rate and gaps; no claims about arbitrary vendor BPE tokenization.
- Include the whole streamed round and a bounded 100ms completion tail, no cherry-picked frame window. No valid samples / incomplete response / wrong input rate cannot pass.
- Cloud Xvfb + software Vulkan results describe that environment, not the specified Ubuntu i5-8250U machine.
- Default application has no profiling feature or automatic test workflow.

## Review focus
- Slow or zero-token input must not falsely pass the 50/s gate.
- Required completion/tool evidence must not be inferred from an early process exit.
- Report contains metrics only and cannot overwrite user data or attach to a user's remote backend.
- Unmeasured/mismatched Draw/Present samples must be counted, not assigned zero.
- Disconnects/timeouts/runner errors must leave no owned app/backend/model process.

## Task 1: reproducible model workload
- [x] Add fixed Markdown token fixture and deadline-paced async emitter; test token reconstruction/rate accounting, disconnect, and invalid options RED→GREEN.
- [x] Retain existing real Read tool round. Extend fake-model stats with actual perf-stream timings, never read the fixture in the model server.
- [x] Commit workload and evidence contract.

## Task 2: optional native capture and report runner
- [x] Add perf feature forwarding Kit profiler; capture actual controller events and GPUI Draw/Present pairs. Test sample/percentile/invalid report semantics RED→GREEN.
- [x] Benchmark configuration requires isolated empty config/workdir and a loopback test endpoint; auto-configure and submit through the existing controller; bounded completion and shutdown.
- [x] Node runner creates fixture/data directories, launches app, validates exit/native/model evidence; outcome check unit tests including empty/slow/incomplete cases. Commit.

## Task 3: Linux release baseline and CI
- [x] Release build, 3 measured runs + controlled disconnect; inspect actual native window. Record hardware/software, complete raw reports, all metrics and limits.
- [x] Fix substantiated implementation issues revealed by measurement; repeat only when changed or unresolved concern warrants it.
- [x] Add deterministic Linux CI checks and optional manual software-renderer benchmark workflow; do not gate reference-hardware performance on shared CI.
- [x] Update tasks/development/report, whole-branch independent review, necessary fix pass, push reviewable PR and monitor checks.
