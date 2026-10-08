# Harness contracts

> Status: Current
> Scope: Platform-neutral Harness types, ports, model-facing values and Assistant read projections
> Canonical owners: `src/lib.rs` exports public contracts; the corresponding source modules own their values and conversions
> Update when: A shared Harness contract, public projection or its consumers change

`yss-harness-contract` defines the typed boundaries used by Harness Core,
Application, provider/storage adapters and the native desktop. It has no GUI,
Application or runtime dependency. Runtime authority and lifecycle rules belong
to [Harness Core](../yss-harness-core/README.md).

- `context`, `capabilities` and `gateway` own identities, capability requests,
  results and injected ports; resource modules own their corresponding values.
- `harness`, `agents` and `persistence` own model events, agent outcomes, sessions,
  ordered event envelopes and the tool ledger. These retain internal execution
  and consistency facts needed by their owners.
- `model` owns model-facing inputs, results and public failure classification.
  Internal binding, revision and persistence fields stay out of this boundary.
- `models` owns provider/model configuration and validation. The credential scope
  matches account ID, provider name and adapter; custom display names, endpoints
  and protocol edits within that scope do not select a different stored key.
  Application enforces this scope before saving or discovering with a saved key;
  desktop forms use the same predicate for connection readiness and feedback.
- `assistant` owns the read projections used by the
  [GPUI Assistant](../yss-desktop-gpui/README.md). It reads existing events and
  records without adding state, subscriptions or another transport layer.

## Assistant read projections

`AssistantEvent` and `AssistantEventKind` project committed Harness events for
live delivery and replay. They retain sequence, session and turn identity,
turn options, model identity, resource references, tool lifecycle, workflow
facts, reasoning and token usage. Nested agent events use the same projection.
Context compaction exposes progress, streamed reasoning/text and the completed
summary. Checkpoint prefix hashes and verification metadata stay inside Harness.
`ContextCompactionDelta` keeps summary-model output separate from the ordinary
reply, including partial output before cancellation or a provider failure.

`AssistantToolIdentity` distinguishes capability tools from agent control tools.
Event failure codes and inspection failures reuse `model`'s public failure
projection, retaining business classifications without internal revision or
project-binding details.

`AssistantToolInspection` reads the existing tool ledger or control lifecycle
events. It contains selected operation parameters, target identity, recorded
timing, committed resource changes and results. Its on-demand `output` uses the
same explicit `model::capability_result` projection as the model, rather than
serializing the internal receipt. Raw request payloads, row values, document
bodies and credentials do not become tool-card parameters.
`AssistantResultReference` preserves result IDs as decimal strings when copied
to JSON, including the full u64 range.

The data flow is `Harness events/ledger → Assistant read projections → native
transcript/tool cards`. GPUI owns display state and interactions; Harness keeps
the original execution, persistence and replay authority. The desktop calls
typed Application use cases directly.

## Validation

Run from the repository root:

```sh
cargo test -p yss-harness-contract --lib assistant::
cargo clippy -p yss-harness-contract --lib --no-deps -- -D warnings
cargo check -p yss-desktop-gpui --bin yss-desktop-gpui
```

The Assistant contract tests cover public failure classification, sanitized tool
inspection, control-event replay, full-width result identities and event
serialization. Changes to other contracts select their affected tests and
consumers separately. Native Assistant interactions use manual acceptance.
