# Protocol codegen feasibility implementation plan

> Use superpowers:executing-plans inline; original task authorization includes task commits and push.

**Goal:** Complete T-0-07 on Linux: derive SessionEvent, Message and Session from the pinned upstream TypeScript and deserialize real backend traffic.
**Architecture:** locked ts-json-schema-generator extracts draft-07 schemas; xtask uses typify to emit deterministic Rust; checked-in schemas/types and a non-mutating --check detect drift. Keep the existing prototype UI boundary until the complete reducer migration, checking both boundaries on the same real traffic.
**Tech stack:** Node >=22, exact npm lock; Rust 1.99.1, typify stable, serde.
**Spec:** docs/spec/02, 05, 06 (T-0-07), 07.

## Constraints
- Upstream mkagent remains at 6e8ebc4, without edits or installing its whole application dependency tree.
- Optional properties map to Option with serde defaults; added object fields are compatible; wire discriminator and retry phases remain precise.
- Generated data cannot silently substitute missing required fields or unsupported schemas with Value.
- Linux is the only active platform verification target; macOS and Windows checks are deferred by user instruction.
- No claim that all stage-zero gates or full T-1-01/channel generation are complete.

## Review focus
- Known malformed event must fail, not be reclassified as an unknown event.
- Retry has two shapes sharing the same type discriminator.
- Upstream added object fields must not break DTO decoding.
- --check must detect drift without modifying repository artifacts.
- Generated schemas/types must have reproducible output with provenance, not machine paths/timestamps.

## Task 1: TS schema extraction
Files: tools/codegen/{package.json,package-lock.json,tsconfig.json,input.ts,schema.mjs,schema.test.mjs}; schemas/*.json.
Interface: node tools/codegen/schema.mjs --out DIR → stable Session/Message/SessionEvent schemas, source metadata; failure exits nonzero.
- [ ] Write extraction contract tests and observe missing generator failure.
- [ ] Install locked isolated generator; resolve only upstream type aliases; generate permissive-object schemas retaining required fields and discriminators.
- [ ] Verify fields, enums, both retry shapes, deterministic output; commit.

## Task 2: Rust generation and drift check
Files: xtask/src/codegen.rs, xtask dependencies/CLI; protocol/src/generated.rs and tests; overrides documentation if needed.
Consumes Task1 schemas. Produces cargo xtask codegen [--check], public generated::{Session,Message,SessionEvent}.
- [ ] Write CLI and Rust decoding tests and observe RED.
- [ ] Generate all reachable DTOs with typify; narrowly adapt any unsupported cases with documented provenance.
- [ ] Test required fields, unknown object fields, optional fields, snake-case event tags and retry phases; two generations stable; --check doesn't write; commit.

## Task 3: Real contract and Linux CI
Files: sidecar/tests/agent_turn.rs, .github/workflows/ci.yml, docs/development.md/tasks.md/verification-codegen-2026-10-29.md.
Consumes generated types from Task2, actual backend RPC values/events.
- [ ] Decode actual Session snapshots, Message values and all observed stream/tool/terminal events with generated types, alongside existing UI reducer; cancel still works.
- [ ] CI Linux installs generator with npm ci and checks drift; core, native and contract jobs target Linux.
- [ ] Run fmt/core/Node/generated/native checks and audit; fresh whole-branch review; fix substantive findings, commit, push PR, monitor CI.
