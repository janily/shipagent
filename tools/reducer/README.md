# Reducer upstream oracle (T-1-06)

`oracle.mjs` executes the eight actual `handlers/__tests__/*.test.ts` files in pinned mkagent v0.2.6 (`6e8ebc456b6b82441be09118ca11afca79b84836`). It transpiles their pure processor/handlers and runtime helpers using the locked TypeScript dependency from `tools/codegen`; it does not reimplement the upstream reducer. A small `bun:test` assertion adapter runs the original 31 tests. Supplemental sequential scenarios cover metadata/effects, background tasks, lifecycle, and reviewed edge cases. Every outer reducer/handler invocation records immutable input, event, explicit clock/generated ID, full output and effects.

```sh
npm ci --ignore-scripts --prefix tools/codegen
node tools/reducer/oracle.mjs         # intentionally regenerate checked-in fixture
node tools/reducer/oracle.mjs --check # compare without writing (CI)
cargo test -p agentkit-store --locked
```

The artifact records the upstream pin, hashes of every loaded source and local oracle input, original assertions and case names. The check rejects dirty/wrong upstream sources and fixture drift. Rust has one named test per original case in eight corresponding modules, plus supplemental and ingress/error guards. Each captured call checks deterministic output, input immutability, full typed state/effect equality and field retention. The checked-in fixture permits offline Rust tests; regenerating it requires the initialized pinned submodule and codegen dependencies.

Upstream renderer tests use `as any` and omit required Session workspace metadata/processing flags and Message timestamp/content. The oracle supplies those fields only in test inputs. Production generated DTO requirements are unchanged. One original annotation fixture still uses obsolete `motivation`, absent from the pinned AnnotationV1 wire schema (which uses `intent`). Its native upstream assertions still run. Rust field auditing allows this one legacy field only and rejects any other silently dropped input/event/expected field.

`Context` supplies an integral millisecond clock and a reserved, nonempty message ID. Callers must reserve a unique ID per event that creates a new message; replay uses the same context. The oracle rejects multiple allocations in one upstream reduction. Narrow serde adapters bridge generated nested DTOs with equivalent schemas and return an error on incompatible upgrades. Invalid integral Message timestamp projections fail without mutating input.

The formal reducer is exercised by the real Agent contract alongside the phase-zero `ChatState`. Compact backend transcripts omit tool messages: live tool identity/result are compared against actual wire events, final assistant ID/content against durable history, and cancellation against the terminal state. GPUI Entity routing and streaming fast paths remain T-1-08; turn grouping is T-1-07.

CI also runs pinned cargo-llvm-cov 0.6.21 with llvm-tools-preview, then `node tools/reducer/coverage.mjs target/reducer-coverage.json`. This enforces 100% instrumented line coverage across the five formal reducer files and uploads the full report; it does not claim branch/region coverage or legacy ChatState coverage is 100%.
