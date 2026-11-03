# Turns oracle (T-1-07)

`oracle.mjs` executes all seven actual chat test files from pinned mkagent v0.2.6 (`6e8ebc456b6b82441be09118ca11afca79b84836`). It transpiles the upstream pure code with locked TypeScript, freezes Date.now, seeds Math.random and uses UTC. It records every exported pure function call with full arguments/results and hashes all loaded source files and local inputs. Original assertions run through the small bun:test adapter; upstream functions are not replaced with our implementation. Native Sets are recorded as arrays, and nonfinite and signed-zero scalar inputs as explicit `$number` markers. No production wire requirements are relaxed.

```sh
npm ci --ignore-scripts --prefix tools/codegen
node tools/turns/oracle.mjs         # intentionally regenerate checked-in fixture
node tools/turns/oracle.mjs --check # CI read-only drift check
cargo test -p agentkit-store --test turns --test turns_guards --locked
cargo llvm-cov -p agentkit-store --tests --json --output-path target/store-coverage.json
node tools/reducer/coverage.mjs target/store-coverage.json turns
```

The seven files are five turns suites, one message timestamp test and `accept-plan-chevron-group.test.ts` (two React CSS source guards). All 81 upstream tests execute. The 79 pure cases have named Rust counterparts in six modules; a case-accounting test ensures the only original cases without captured function calls are the two explicitly identified CSS guards. Those CSS source files are hashed and their original assertions run in CI. This proves upstream provenance, not GPUI button interaction; native plan-button acceptance remains the relevant UI task.

The Rust harness compares complete projected values, treats integral/floating JSON numbers semantically, sorts only Set results, audits every typed input field, and checks deterministic output. The API borrows messages/activities immutably. Supplemental cases exercise lifecycle boundaries, hidden messages, latest todos/interruption, bounded/cyclic ancestry, continued groups, TaskOutput parsing, identity, selectors, and exact JS formatting edges. Strict discriminator enums keep system/user/assistant/group payloads distinct.

`format_message_timestamp(timestamp_ms, offset_seconds)` takes the historical local offset from its caller, including DST. The function performs no timezone lookup or clock IO; out-of-range calendars/offsets return None. Native timezone resolution is an application capability boundary, not evaluated by the UTC oracle. Malformed TodoWrite view-model arrays are omitted; generated Messages and their tool-owned input are never rewritten. Todo status strings remain tool-owned and preserve future status names.

This module implements spec03's turns grouping, phase, activity hierarchy, summaries, formatting and supporting selectors/identity. Monaco-specific Markdown detail export and stored-message conversion are not needed for this native core task; generated Message is already the input. Current ChatState/UI stays compatible until T-1-08 integrates entities and event routing. Both reducer and turns line coverage are enforced at 100% in CI; branch/region coverage is reported separately without a 100% claim.
