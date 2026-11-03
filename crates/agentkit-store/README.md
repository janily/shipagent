# agentkit-store

`core::reducer` is the formal, generated-DTO session event reducer. `process` and `process_decoded` borrow immutable input and return a new `ProcessResult` with typed state and ordered deferred effects. The explicit `Context` carries time and a reserved message ID, keeping reduction independent of IO, GPUI, clocks and randomness. Unknown decoded event tags and events from other sessions leave state unchanged. Invalid Message timestamp projections and generated nested DTO incompatibilities return `ReducerError`.

See the [upstream oracle](../../tools/reducer/README.md) for provenance, fixture generation and tests. The phase-zero `ChatState` remains the app compatibility projection until T-1-08 migrates entities/event routing. This task does not claim that migration or stage-zero acceptance.
