# `yss-api` transport contract

> Status: Current
> Scope: Tauri command、event、channel、wire DTO 和 transport error seam
> Canonical owners: `yss-api` source/tests own executable wire facts; this README owns the transport contract
> Update when: public transport shape、registry ownership、delivery semantics 或 frontend invoke seam 改变时

`yss-api` is the only Tauri transport seam in YssBI. It owns command handlers, wire DTO mapping, transport errors, event/channel delivery, and the canonical command registry. It does not own Project, Graph, Database, Execution, SCI, Harness, or Workbench behavior.

## Public surface

The crate keeps commands, schemas, transport errors, and event adapters private. Its production surface is intentionally narrow:

- `invoke_handler()` builds the single command registry consumed by `src-tauri/src/lib.rs`;
- injected runtime state types are exported only when the composition root must construct them.

The composition root constructs authorities and adapters, then injects them. It must not declare a second command registry, command schema module, or transport error type.

## Command responsibilities

A command handler may:

1. parse and validate IPC input;
2. convert wire DTOs to application/domain types;
3. call one application/domain use case;
4. move blocking work to the established blocking boundary when needed;
5. map typed outcomes to wire DTOs or `CommandError`;
6. deliver an event/channel message after the authoritative operation reaches its documented commit point.

A command handler may not own filesystem transactions, long workflows, duplicated domain validation, Project/Graph reconciliation, statistical computation, or durable state. A delivery failure can produce a transport failure, but it does not pretend that an already committed authority mutation never occurred.

## DTO ownership

Wire DTOs are explicit transport types. Internal structs are not exposed merely because they implement serialization. Mapping is owned at this seam; domain/application crates do not depend on Tauri or frontend wire schema.

Use stable camelCase fields and strict frontend parsers for public DTOs. A contract change updates the Rust mapper, TypeScript parser/types, representative boundary tests, and the owning architecture document together. YssBI 0.x contracts are migrated directly unless compatibility is an explicit product requirement.

## Commands, events, and channels

Choose the transport by semantics:

| Primitive      | Use                                                                                       |
| -------------- | ----------------------------------------------------------------------------------------- |
| Command        | bounded request/response work with one typed outcome                                      |
| Event          | low-rate state-change notification that does not carry authority                          |
| Channel/worker | ordered, streaming, high-frequency, progress, execution, diagnostics, or Harness delivery |

Every ordered stream defines its source identity, ordering key, capacity/backpressure behavior, loss/gap handling, replay or snapshot recovery, cancellation, and terminal semantics in its domain owner. `yss-api` maps that contract to Tauri without inventing a second queue model.

Events are notifications, not state stores. Consumers recover authoritative data through the domain’s snapshot/query command rather than rebuilding it from an assumed complete event history.

## Error contract

Every command rejection serializes the Rust-owned `CommandError` with exactly three camelCase keys:

```json
{
  "code": "project_not_found",
  "details": null,
  "incidentId": null
}
```

- `code` is a stable lower_snake_case machine category;
- `details` is `null` or a safe structured object, never raw/internal prose;
- `incidentId` is always present and is `null` unless diagnostic correlation is required.

The wire never contains a backend-owned `message`. Do not encode identity in string prefixes, return `Result<T, String>` from a command, or accept legacy error shapes. Expected failures map to stable code/details. Internal or infrastructure failures generate an incident identity and write technical context only through sanitized tracing/diagnostics.

Successful DTOs and asynchronous statuses may not bypass this rule with backend prose fields such as `message`, `detail`, `hint`, or `reason`. Domain diagnostics that are intentionally user-visible use a stable code, safe location/parameters, and their domain-owned deterministic template contract.

Frontend application code localizes `code + safe details`; `IpcError.message` is a technical summary and must not be rendered directly.

## Frontend adapter

Ordinary frontend invocation goes through `src/services/ipc/invokeCommand.ts`, which validates the common error wire. Domain services under `src/services/` own command-specific request/result parsing. Views and presentation modules do not call Tauri `invoke` directly.

Channel adapters likewise parse strict wire DTOs before publishing to application projections. A malformed payload or sequence gap is handled as a transport/recovery failure, not silently accepted into domain/UI authority.

## Data and security boundary

- Keep large datasets and computation in Rust; expose paging, projection, batching, handles, or result IDs.
- Do not send raw infrastructure errors, SQL, connection strings, credentials, prompts, transcripts, document content, clipboard content, or table rows through error details or diagnostics fields.
- Treat resource paths and IDs as opaque values; do not normalize domain identity in transport/UI code.
- Credential configuration uses explicit injected/application paths and is never persisted in Harness/Project/logging by this crate.

## Related owners

- [System architecture](../../../docs/architecture/ARCHITECTURE.md)
- [Graph and Execution](../../../docs/architecture/GRAPH_AND_EXECUTION.md)
- [Runtime Signals](../../../docs/architecture/RUNTIME_SIGNALS.md)
- [Statistical Harness](../../../docs/architecture/STATISTICAL_HARNESS.md)
- [Change Process](../../../docs/development/CHANGE_PROCESS.md)
- [Local Workflow](../../../docs/development/LOCAL_WORKFLOW.md)

The exact command list is executable source in `src/lib.rs` and must not be copied into this README.
