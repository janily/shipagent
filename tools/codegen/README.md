# Protocol generation (T-1-01 / T-1-03)

```sh
npm ci --ignore-scripts --prefix tools/codegen
cargo xtask codegen
cargo xtask codegen --check
node --test tools/codegen/*.test.mjs
```

Requires Node >=22 and the pinned Rust toolchain including rustfmt. CI checks out submodules and runs `codegen-drift`. Extraction reads TypeScript declarations and literal AST nodes; it never imports or executes upstream application modules. `skipTypeCheck` omits unrelated application diagnostics, not reachable type-resolution errors.

`sources.json` explicitly lists 35 safe wire roots and 4 schema-only sensitive roots and the reviewed upstream commit. Generation requires the submodule HEAD and staged gitlink to match that commit and rejects dirty/untracked (non-ignored) backend files. Upgrade the gitlink and manifest together in a dedicated reviewed upgrade. This is a source guard, not a claim to typecheck the upstream application.

The roots cover Message, Session, SessionEvent, Workspace, permissions, session creation/commands/send options, attachments, modes and role/status enums. Identical nested shapes reference their canonical root, so one override affects every occurrence. Credential-bearing setup/OAuth/proxy DTO schemas are audited but never fed to typify; manual SecretString API wrappers preserve redacted diagnostics. The generator rejects sensitive roots reachable from generated DTOs. This is not the entire upstream exported type graph.

`overrides/schema.json` contains exact JSON pointers, expected original schemas, replacement schemas and reasons. Missing paths, changed original shapes and duplicates fail before artifacts are published. Signed epoch timestamps use i64; attachment byte counts use u64; messageCount uses u32. Durations, elapsed seconds, costs, token usage and other unreviewed TS numbers retain f64; do not infer that every numeric field is an integer. Upstream source declarations take precedence over illustrative spec snippets (for example required workspaceName/messages and canAlwaysAllow).

Checked-in schemas are **effective schemas after overrides**. `schemas/manifest.json` records source/config/tool input hashes and the extracted schema/constants artifact hashes; the constants digest matches the formatted `src/constants.rs`. Rust DTO generation is additionally checked byte for byte by `--check`. The manifest has no machine paths or timestamps. Typify and Rust dependencies are fixed by Cargo.lock; TypeScript/extractor dependencies by package-lock.json.

`SessionEvent` variants use discriminator-based names (`TextDeltaEvent`, `RetryBackoffEvent`, `RetryStateEvent`), independent of upstream union order. New conflicting event shapes fail closed. `DecodedSessionEvent` validates known tags against these generated types and retains complete raw payloads, including future fields/tags. The running prototype event entry now uses this validator too. The prototype Message/Session structs remain temporary UI/domain projections; the typed API is implemented in T-1-03; complete domain/reducer migration belongs to T-1-06.

The generated `constants::{channels,limits,PROTOCOL_VERSION}` comes from upstream channels.ts/types.ts/message-limits.ts. The static parser permits strings, object literals, integer literals/references and multiplication only; unknown expressions fail rather than executing. Namespace keys become snake_case Rust modules; wire strings remain unchanged. Current WorkbenchApi uses generated channel constants. The typed API covers the v1 request/event channel table; runtime handshake availability still applies, including pending backend patches.

Mechanical Rust adaptations preserve wire semantics:

- Convert draft-07 const to singleton enum before typify, retaining precise discriminators.
- Wrap optional collections in Option, preserving absent versus explicit empty arrays/maps.
- Required nullable fields use a deserialize hook to reject absence but accept explicit null. Optional fields use Serde Option semantics (null becomes None).

`--check` extracts/generates in a temporary directory and reports changed, missing or obsolete artifacts without writing the worktree. Normal generation removes obsolete JSON schemas in the owned schemas directory. Extraction/override/type-generation failures publish nothing. The backend checkout is never modified.

Recursive SessionFile/SkillFile trees explicitly expose exports for extractor references. `overrides/api-input.ts` rewrites the unsupported inline WorkspaceSettings import into direct imports; generation audits every field signature, optionality and heritage against the original interface. Other extraction failures remain fatal. SessionBundle version remains integer literal 1 in the effective schema.
