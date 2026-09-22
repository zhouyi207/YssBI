# Shared desktop IPC contract

> Status: Current
> Scope: Shared wire DTOs, event envelopes and command error payloads
> Canonical owners: This crate owns shared serialization; adapters own conversion from application/runtime facts
> Update when: A shared IPC payload changes

`yss-ipc-contract` defines project receipts, graph projections, Harness messages, execution stream values, project progress and `CommandErrorDto`. Command, Event and Channel consume these same types.

Node editor capabilities contain only the required `managed` boolean. Copy, duplicate, delete and cut availability derive from this ownership flag; the node capability payload rejects missing or unknown fields. Parameter and inline-literal editors consume their own existing projections.

Graph editor sessions require `document`, `projection`, `editing`, and `resultState`. The result summary carries the matching semantic input hash, execution session ID and a decimal-string `revision` independent of the edit revision. Snapshot and delta delivery carry the same complete contract. Clients atomically publish graph entities, editing metadata and result validity, and reject older result summaries for the same execution session and semantic identity.

The crate has no Tauri, Application, database or execution runtime dependency. It reuses neutral identity/document contracts and existing projection value types, including their value conversions. It creates no subscriptions, tasks, caches, incident records or business state. Application mappings and incident recording remain in their adapters.

Execution demands use `default` or `outputs`; run events describe run lifecycle and result inspection requests. Pin View reads current results through result queries and has no separate execution demand, generation allocator or completion event.
An `outputs` demand includes `includeDefaultResults` and the required boolean `reuseInputs`. Report extensions set `reuseInputs` to true to reuse valid upstream inputs while recomputing requested outputs; ordinary output runs set it to false. Execution owns cache validation and publication checks.

Command-only request/response schemas may stay beside their handler. Shared types have one definition here, with no compatibility re-export from the former schema modules.

Harness tool lifecycle events are `tool_invocation_started`, `tool_invocation_completed`, and `tool_invocation_failed`. Each carries its actual invocation identity and capability ID; there is no pre-identity placeholder event or legacy event conversion.

The complete wire and delivery contract is maintained in [Desktop IPC](../yss-application/src/ipc/README.md).
