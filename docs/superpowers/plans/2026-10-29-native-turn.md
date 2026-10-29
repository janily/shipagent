# Native streaming and complete Agent turn implementation plan

> Execute inline using superpowers:executing-plans. Original task authorization covers task commits and push; no design change or approval gate is needed for the already specified T-0-05/T-0-06.

**Goal:** A real GPUI Kit workbench sends a message through bundled mkagent and displays streamed text, a tool call/result, and the completed answer.
**Spec:** docs/spec/01–07, especially U2/U3 and T-0-05/T-0-06.
**Architecture:** prototype DTOs in protocol; typed API in client; pure event reduction in store; GPUI view emits intents; app owns backend/RPC, event routing and shutdown. A scripted local OpenAI-compatible fake model validates real backend behavior without external model credentials.

## Constraints
- Keep pinned versions, source submodule clean, no tokio direct dependency.
- No simulated tool results in UI, no credentials in logs, no automatic switch to allow-all.
- Handwritten prototype DTOs are explicitly not the final codegen deliverable.
- Match uploaded workbench layout; all owned UI copy goes through zh/en locale keys, semantics through Kit theme tokens.
- No macOS signing/clean-machine or 60fps performance claims without actual evidence.

## Review focus
- User messages are not duplicated when optimistic insertion meets user_message.
- Old-session or cancelled-turn text cannot corrupt the newly active conversation.
- Send failures preserve the draft and stop busy state; socket loss is visible.
- Permissions need explicit user choice; peer cancellation and shutdown do not leave a backend.
- Local model actually receives a tool result before producing final answer.

## Task A — typed boundary and reducer (T-0-05/T-0-06 prerequisites)
Files: protocol/src/prototype.rs; client/src/api.rs; store/src/core.rs; store/tests/streaming.rs.
Produces: Workspace/Session/Message/SessionEvent DTOs; WorkbenchApi facade; ChatState::apply and optimistic lifecycle.
- [x] Write replay tests for text delta/complete/discard, optimistic user replacement, tools, permission, interruption and session isolation; observe RED.
- [x] Implement only supported prototype events, preserve unknown raw payload; keep pure state independent of GPUI.
- [x] Typed methods for workspace/session bootstrap, send/cancel/permission/model connection setup; exact positional args from upstream.
- [x] Core tests/fmt/clippy and task commit.

## Task B — local fake model and real full turn (minimal T-0-08 supporting T-0-06)
Files: tools/fake-llm/server.mjs and tests; client/tests or app integration harness.
Produces: loopback HTTP SSE endpoint, text → read tool call → final text based on actual read result.
- [x] Test SSE framing, tool-call finish_reason, and rejecting a second response without tool result.
- [x] Start real bundled backend with isolated config; configure custom OpenAI endpoint via RPC, create safe session and send.
- [x] Assert actual tool_start/tool_result and final persisted messages; cancellation and failures do not count as complete success.
- [x] Bound all waits and clean up processes; commit evidence, not credentials or session contents.

## Task C — native UI and controller (T-0-05/T-0-06)
Files: ui/src/{workbench,chat,composer}.rs; app/src/{lib,controller,brand}.rs; apps/workbench/src/main.rs; locales and UI tests.
Produces: runnable native workbench, virtual MessageScroller, incremental TextViewState, native Textarea, send/stop, session switching/new, permission prompt, truthful connected/loading/error states.
- [x] Create headless UI tests: reducer-fed stream updates, input submission/stop and permission intents, final rendered answer; observe RED.
- [x] Build views with Kit controls and semantic tokens; preserve scrolling and cached Markdown per message.
- [x] App asynchronously starts sidecar or uses explicitly configured local remote endpoint, binds workspace, bootstraps sessions and consumes events in <=16ms batches.
- [x] Hook quit to cancel and shutdown; no synchronous IO in render/main GPUI thread.
- [x] Run full native check/build, UI tests and real backend tests, inspect native screenshot if environment supports it.
- [x] Record timings as measurements (not benchmark success), commit and push a reviewable PR.

## Remaining phase-zero gates
T-0-07 full codegen feasibility, T-0-09 macOS signing and clean-machine validation, T-0-10 broader component samples and T-0-11 report remain independent. Completing this plan does not itself pass every phase-zero gate.
