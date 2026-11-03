# T-1-09 event fixture recording and replay

Base e5d8b75 (PR15 merged). Apply spec05 recording contract and spec06 T-1-09. No stage0/release acceptance.

1. Isolated real sidecar + script model recorder via cargo xtask fixtures record <scenario|all>, subscribed before session creation. Keep event JSONL, initial/final authoritative snapshots, version/provenance and scenario evidence. Bounded deadlines, private configuration, deterministic local fixtures, explicit cleanup. Never synthesize session events to satisfy a scenario.
2. Capture all14 required scenarios. Verify scenario signatures, including nested children, permission decisions, real retry/typed error, plan, compaction, background completion. If upstream semantics differ, make limits/projection explicit instead of mutating recorded wire events.
3. Pure default offline replay against persisted durable message projection, plus indexed cache consistency with full turns. Negative replay/provenance checks. Add fresh three-platform capture+replay CI and developer recording docs/env hook. Independent standards/spec review, full checks, commit/push/ready PR; no merge.
