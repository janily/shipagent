# Component samples and platform verification implementation plan

> Execute inline with superpowers:executing-plans; continuation already authorized.

**Goal:** T-0-10 six interactive Kit samples and native macOS/Windows validation.
**Architecture:** An independent UI example uses no backend, credentials or user settings. Existing production conversation flow stays separate. CI runs native hosts, not cross compilation.
**Tech Stack:** Rust 1.99.1, GPUI Kit =0.8.1, Bun 1.3.14, Node 22.
**Spec:** docs/spec/04 界面与交互规格.md; docs/spec/05 工程规范与质量.md; docs/spec/06 任务分解与排期.md.

## Global constraints
- Keep pinned backend and dependencies unchanged; patches stay outside submodule.
- macOS/Windows automated build/test evidence is distinct from physical desktop, signing and clean-machine acceptance.
- Samples use semantic Kit themes and actual Sidebar/Textarea/Collapsible/Settings/Dialog/Resizable.

## Review focus
- Sample must never launch backend or persist user settings.
- Draft survives switching settings; blank submissions do not erase it.
- Dialog cancel must not clear content; confirm does.
- Resize minima keep both panels usable; input is reachable at 960x600.
- Native CI failures remain visible; no continue-on-error or unsupported-platform success claims.

### Task 1: Native host verification
Files: .github/workflows/ci.yml, docs/development.md.
Interfaces: existing cargo check/test and xtask backend build.
- [x] Expand unit/native/real-backend contract matrices to ubuntu-latest, macos-14, windows-2022 with fail-fast false; Linux dependencies only on Linux; Linux-only supervisor tests conditional.
- [x] Commit/push, create draft PR, observe actual host results and fix reproducible portability failures.
Expected: each matrix job succeeds, or exact failed commands/errors recorded pending fixes.

### Task 2: Interactive component sample
Files: crates/agentkit-ui/src/samples.rs, examples/components.rs, tests/samples.rs, lib.rs.
Produces: ComponentSamples::new(window, cx), standalone cargo run -p agentkit-ui --example components.
- [x] Native tests first: type Chinese and submit; blank ignored; settings round trip preserves draft; collapse shows/hides detail; clear dialog cancel preserves and confirm clears. Observe missing API failure.
- [x] Implement actual six Kit components, local state only, settings theme switch immediate, constrained two-pane resize.
- [x] Run native tests and existing suite; run example on Linux Xvfb, inspect/capture screenshot and resize.
Expected: tests pass, sample shows actual controls at 1280x820 and 960x600.
- [x] Commit and push.

### Task 3: Evidence and review
Files: docs/verification-components-platform-2026-10-29.md, docs/development.md, README.md.
- [x] Record actual native CI results with links, sample command and Linux screenshot, pending physical desktop/signing/performance gates.
- [x] fmt, core/native tests, all-features native check/clippy, codegen drift and CI.
- [x] Fresh whole-branch review; fix important findings, commit/push and mark PR ready when checks pass.
Expected: reviewable PR, no phase-zero acceptance claim.

Execution note: real native smoke was strengthened after review to require a profiler Present record from the matching window, after a second frame callback. Node reports reject callback-only evidence. Windows loopback buffering required splitting deterministic AsyncWrite backpressure coverage from real-TCP deadline/explicit-close coverage. CI runs on PR/main push to avoid duplicate matrices. Two sample-only minor findings remain documented in the verification report.
