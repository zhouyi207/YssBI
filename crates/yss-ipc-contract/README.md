# Native Assistant projections

> Status: Current
> Scope: Public Assistant events, tool inspection and result references consumed by the native desktop
> Canonical owners: `src/harness.rs` and `src/harness/inspection.rs` own these read projections; Harness contracts own the underlying facts
> Update when: A public Assistant projection or its native consumer changes

`yss-ipc-contract` supplies the public read projections used by the
[GPUI Assistant](../yss-desktop-gpui/README.md). The native host calls typed
[Application use cases](../yss-application/README.md) directly for project,
graph, execution, model configuration and conversation operations. This crate
owns no command requests, transport, subscriptions or business state.

`HarnessEventDto` projects committed Harness events for live delivery and replay.
It retains sequence, session and turn identity, turn options, model identity,
resource references, tool lifecycle, workflow facts, reasoning and token usage.
Nested agent events use the same projection. Context compaction exposes progress
and occurrence while keeping checkpoint text and prefix hashes inside Harness.

Tool failure codes and inspection failures reuse the Harness contract's public
failure projection. Resource verification and detected conflicts keep their
business classification; internal revisions and project-binding details stay
out of failure payloads.

`HarnessToolInspectionDto` reads the existing tool ledger or control lifecycle
events. It contains selected operation parameters, target identity, recorded
timing, committed resource changes and results. Raw request payloads, row values,
document bodies and credentials do not become tool-card parameters.
`HarnessResultReferenceDto` preserves result IDs as decimal strings when copied
to JSON, including the full u64 range.

The crate depends only on Serde, JSON values and the neutral Harness contract.
It has no Application, graph runtime, database, GUI or platform dependency.

## Validation

Run from the repository root:

```sh
cargo test -p yss-ipc-contract --lib harness::
cargo clippy -p yss-ipc-contract --lib --no-deps -- -D warnings
cargo check -p yss-desktop-gpui --bin yss-desktop-gpui
cargo fmt -p yss-ipc-contract -- --check
```

Contract tests cover public failure classification, sanitized tool inspection,
control-event replay, full-width result identities and event serialization.
Native Assistant interactions use the host's manual acceptance.
