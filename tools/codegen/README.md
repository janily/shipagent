# Protocol generation (T-0-07)

```sh
npm ci --ignore-scripts --prefix tools/codegen
cargo xtask codegen
cargo xtask codegen --check
node --test tools/codegen/schema.test.mjs
```

Requires Node >=22 and the pinned Rust toolchain including rustfmt. `--check` generates in a temporary directory, compares all three schemas and Rust output, and never rewrites tracked artifacts. CI must check out submodules. No backend application code is executed by extraction, and the backend working tree is not modified.

The upstream public type barrels are the input. We disable unrelated application TypeScript diagnostics (`skipTypeCheck`), but unresolved reachable types and unsupported schemas still fail generation. TypeScript runtime typechecking is not claimed. `expose:none` avoids the public/core PermissionRequest name collision by inlining reachable shapes; structurally identical Message objects become references to the generated Message definition.

Three compatibility adaptations are mechanical and tested:

- ts-json-schema-generator emits draft-07 `const`; typify currently treats those as unconstrained strings in some unions. Convert each literal to its semantically identical singleton `enum` before invoking typify. The checked-in schemas remain the original extraction.
- typify maps absent optional collections to empty Vec/Map. A Rust AST pass wraps those optional fields in Option and skips only None, preserving absent versus explicit empty values. Required collections remain required.

- Required nullable properties also use Option in Rust, but receive a deserialize_with hook so a missing key is rejected while explicit null remains valid. Optional properties intentionally use Serde Option semantics (explicit null is accepted as None).

`generated::{Message,Session,SessionEvent}` is a feasibility surface. Event variant names are typify's unstable `VariantN` where duplicate `retry` tags prevent a simple tagged union; use the validated `DecodedSessionEvent` boundary rather than relying on variant indices. That boundary retains complete raw data, accepts future event types and rejects malformed known types. The supported tag list is generated from the schema, not maintained by hand.

TS `number` becomes f64 because the source does not specify an integer domain. Integer-only timestamps/counts and final ergonomic naming need reviewed schema/type overrides during T-1-01; the generated DTOs currently validate alongside the existing prototype UI boundary. Full channel/constants generation and replacement of every handwritten DTO are outside T-0-07.
