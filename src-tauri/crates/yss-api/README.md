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

Native window geometry uses the official Window State plugin registered by the composition root.
There are no YssBI window-state query/save commands or geometry DTOs. The frontend creates hidden
windows through the platform adapter and the Rust plugin owns restoration and persistence;
see [Workbench window geometry](../../../docs/architecture/WORKBENCH_DOCKVIEW_ARCHITECTURE.md#81-原生窗口几何与关闭).

## Command responsibilities

A command handler may:

1. parse and validate IPC input;
2. convert wire DTOs to application/domain types;
3. call one application/domain use case;
4. move blocking work to the established blocking boundary when needed;
5. map typed outcomes to wire DTOs or `CommandError`;
6. deliver an event/channel message after the authoritative operation reaches its documented commit point.

A command handler may not own filesystem transactions, long workflows, duplicated domain validation, Project/Graph reconciliation, statistical computation, or durable state. A delivery failure can produce a transport failure, but it does not pretend that an already committed authority mutation never occurred.

`ApplicationCapabilityGateway` is the injected scheduling adapter for the internal Assistant capability port. It moves the synchronous Application use case to the blocking pool, enforces the supplied read-only deadline/cancellation budget, and maps worker failures to typed capability failures. Harness continues to own tool admission, ledger, lifecycle events, and turn state; it never calls Tauri commands as its business bus.

Graph tools additionally use the ephemeral `HarnessGraphClientHub` channel because the webview owns unsaved drafts and their FIFO/history. The webview supplies that explicit draft to one Application action; Rust retains the capability receipt and sends an existing draft/compile/save projection for adoption. Claim and completion acknowledge adoption only, never accept model-authored results. Identity/hash checks reject stale or misrouted updates. This is a draft-owner boundary, not a generic command dispatcher.

## DTO ownership

Harness tool start/completion/failure events carry the same invocation ID. Failure events expose only the stable `failureCode`, including cancellation and timeout. Harness channel subscriptions buffer live events until historical replay has been merged, then deliver each sequence once. The frontend consumes this stream as a projection.

Wire DTOs are explicit transport types. Internal structs are not exposed merely because they implement serialization. Mapping is owned at this seam; domain/application crates do not depend on Tauri or frontend wire schema.

Use stable camelCase fields and strict frontend parsers for public DTOs. A contract change updates the Rust mapper, TypeScript parser/types, representative boundary tests, and the owning architecture document together. YssBI 0.x contracts are migrated directly unless compatibility is an explicit product requirement.

Graph documents carry an optional `constants` map keyed by stable UUID. Each constant contains `id`, `name`, `dataType`, Rust-tagged `dataValue`, and optional tabular snapshot, description and tags. `setConstant { id, constant }` replaces a definition or deletes it with `null`; `insertConstantReference { id, position }` inserts its Get node. These are Graph draft mutations and use the existing Transform, Resolve, Save and history contracts. Node parameter editor `graphConstant` selects a definition in the current graph. Clipboard snapshots optionally carry referenced constant definitions in `constants`.

Project queries use `get_project_databases` for database declarations. Variable commands, variable resource deltas and variable collections are removed; Project index contains graph, chart and database resources.

Node parameter editors carry one effective `value`, display/editor metadata, and optional schema-aware `configuration`. Rust resolves that value from the node document or its protocol default. The wire has no project-setting inheritance source or override options; parameter edits update the Graph draft document directly.

Node parameters with `editor: "configuration"` carry `configuration: { kind: "configuration", fields }` containing the active `ParameterEditorDto` fields for a Detail form. Select fields use `configuration: { kind: "selectOptions", options }`. The `setConfiguration` mutation carries `{ nodeId, key, values }`, with partial field values merged, normalized, and validated against the current node parameters by Graph Editor. Configuration objects persist in the node's parameter map and use the existing draft undo/redo and Save path. Static configurations have no input binding, source selector, or connection mutation.

## Commands, events, and channels

`list_sample_datasets` returns a bounded, project-independent list of
`{ id, name, version, rowCount, columnCount, byteSize }`. Resource paths, source hashes
and payload rows remain in Rust. `import_sample_dataset` accepts `projectInstanceId`,
`operationId`, `sampleId` and `version`, invokes one Application use case on the blocking
pool, and returns the existing database mutation aggregate. Its receipt/event uses
the normal Project publication coordinator. Sample errors retain the common error
wire, with stable categories for missing samples, changed versions, invalid catalogs,
integrity failures and unavailable resources. The frontend localizes these categories.

`get_database_rows` returns `{ rows, rowIds }` with both arrays required and the same length.
The frontend keeps these stable row identities and rejects bare row arrays or incomplete pages;
it does not substitute an empty identity array for an obsolete response shape.

`get_result_page` 在 blocking worker 上调用 Application 的页面查询；命令不直接执行关系查询或持有数据库锁。
`ResultPage.totalCount` 为可空整数，未知总数使用 null；`hasMore` 与 `nextOffset` 决定是否可继续翻页。
关系页面的 metadata 为 `{ columns: [{ name, type }] }`，values 为按该列顺序排列的行数组。
超出 JavaScript safe-integer 范围的单元格使用十进制文本，精确存储类型保留在列元数据中。
日期时间单元格使用不带时区的日历/钟面字符串，类型名只包含时间单位；其转换语义由
[Dataset store](../yss-dataset-store/README.md) 维护。

Choose the transport by semantics:

| Primitive      | Use                                                                                       |
| -------------- | ----------------------------------------------------------------------------------------- |
| Command        | bounded request/response work with one typed outcome                                      |
| Event          | low-rate state-change notification that does not carry authority                          |
| Channel/worker | ordered, streaming, high-frequency, progress, execution, diagnostics, or Harness delivery |

Every ordered stream defines its source identity, ordering key, capacity/backpressure behavior, loss/gap handling, replay or snapshot recovery, cancellation, and terminal semantics in its domain owner. `yss-api` maps that contract to Tauri without inventing a second queue model.

Events are notifications, not state stores. Consumers recover authoritative data through the domain’s snapshot/query command rather than rebuilding it from an assumed complete event history.

Graph editor projections currently arrive in load/hydrate/mutation/Compile/Save command responses. There is no Graph Projection subscription channel or invalidation snapshot. Diagnostics and Execution channels are separate contracts; diagnostics live-gap recovery is documented in [Runtime Signals](../../../docs/architecture/RUNTIME_SIGNALS.md#4-operational-diagnostics).

## Activity panel projection

`get_activity_panel_document` accepts Project, Nodes, Commands or Plugins, locale, and explicit project identity
for Project/Nodes. A null project requests empty documents; Commands/Plugins require null.
The invoking WebView supplies the window identity. Cursors and request size are validated at this transport seam.

Normal Project synchronization uses `get_project_index` with locale and Project/Nodes cursor requests.
It returns `{ index, activityPanels }`: the Application captures one ProjectIndex, maps its Project document,
and uses that same index for the Nodes catalog. Version revalidation does not rescan files.
Each requested panel is encoded through the existing ActivityPanelSyncState as snapshot or patch.
The frontend validates the whole response, then installs resource and panel projections through its existing publication owner.
A lost or invalid panel baseline causes one whole-response snapshot recovery; there is no follow-up Activity query driven by ResourceStore.

The exact `yssbi.activity-panel.v1` envelope contains panel/project identity, publication revision,
title, tools, complete depth-ordered rows, and optional empty-state presentation. Text is either
a localization key or a literal. Category defaults and graph, chart, database, node, command and plugin item kinds are Rust-owned. The payload
contains resource references and node creation descriptors, not graph documents, database engines,
connection strings, or table data. Row count, depth and serialized response size are bounded;
invalid/oversized input is never partially rendered.

The first reply is `{ kind: "snapshot", cursor, document }`. Subsequent replies use
`{ kind: "patch", baseCursor, cursor, patch, operations }`. The document-level patch is limited
to title, tools, emptyState and publicationRevision. Row operations are:

| op     | Fields       | Meaning                                                                  |
| ------ | ------------ | ------------------------------------------------------------------------ |
| update | id, patch    | Replace only changed top-level fields of one row; id/kind are immutable. |
| insert | afterId, row | Insert one row after the named row; null means the beginning.            |
| remove | id           | Remove exactly one row.                                                  |
| move   | id, afterId  | Reposition one existing row without replacing its contents.              |

Nested values are replaced as fields, not recursively merged. Null is a value. Row-kind changes
are represented by remove+insert. Batches apply atomically, so parent/child ordering is validated
after all operations. Unchanged rows retain their frontend references; unchanged documents return
empty patches with the original cursor.

`ActivityPanelSyncState` is an injected transport cache, not committed state. It retains immutable,
window/panel/project/locale-bound baselines under explicit count and serialized-byte budgets.
Query/diff work runs outside its lock. Lost replies can still be diffed from a retained cursor;
an unavailable cursor causes a fresh snapshot. Ordinary refreshes never echo a whole tree or
send the client's tree to Rust. This is request/response synchronization driven by existing
invalidation signals, not a new polling or push stream.

The frontend service validates identity, cursor continuity, allowed patch fields, row kinds,
unique ids and final tree depth before publication. Malformed batches do not partially mutate
the visible projection. A failed delta gets one explicit snapshot recovery; a repeated failure
is surfaced. Requests within a binding are serialized and coalesced. Project/language/epoch
replacement invalidates the binding; mounted consumers share the existing sidebarStore document cache. UI expansion creates no request and only expansion preferences are persisted.
See [Workbench](../../../docs/architecture/WORKBENCH_DOCKVIEW_ARCHITECTURE.md) for UI ownership.

Resource command replies and matching `ResourceMutationCommitted` events enter the same frontend
publication coordinator. A committed mutation carries a positive monotonic publication revision and
canonical deltas. Replies, events and watcher invalidations share one serialized authoritative-index installer,
not separate delta and snapshot Store writers. The installer uses receipts for correlation and resource moves,
prepares loaded clean documents, and deduplicates late receipts already covered by its snapshot.
Unchanged indexes do not republish sidebar state. Watcher refreshes do not reactivate the project or rebuild the workbench.

`save_chart` takes project identity, operation ID, resource path and a complete document, with no
`expectedRevision` argument. Chart documents carry no resource revision. `load_chart` accepts an optional
`expectedPublicationRevision` for reads prepared against an authoritative index; Rust rejects a mismatched
project snapshot before returning the document. Save and publication ownership are defined in
[Graph and Execution](../../../docs/architecture/GRAPH_AND_EXECUTION.md#save).

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

Editor diagnostics carry code, messageKey, safe arguments, severity, explicit blocking, location and related locations. Rust owns definitions/templates; React uses the generated vocabulary for localization. This domain fact contract remains separate from CommandError.

Compile returns Ready { artifactId, projection, cacheHit } or Blocked { projection }; neither branch returns a replacement document. Expected semantic failures remain Blocked without diagnostic incidents. Internal failures still reject with the error wire above. The read-only resolve_graph_draft command supplies current projections for dirty refresh and history validation; execute_compiled_graph accepts compiledArtifactId and explicit demand. See [Graph and Execution](../../../docs/architecture/GRAPH_AND_EXECUTION.md).

Frontend application code localizes `code + safe details`; `IpcError.message` is a technical summary and must not be rendered directly.

Harness event types use snake_case while envelope and payload fields use camelCase. Provider turn failures retain stable categories for authentication, rate limits, rejected requests, connection failures, unavailable services and invalid responses. The error wire never includes a raw provider response or credential. See [Statistical Harness](../../../docs/architecture/STATISTICAL_HARNESS.md).

Execution terminal failures carry `RunErrored { code, phase, source }`, where source is null or a safe graph/node/port identity. Numeric failures retain their specific cause. When a rejected execution command has already delivered a terminal event, its details include `terminalRunEventSent: true`; the frontend drains the channel before finalizing the command failure so the Output panel retains the typed cause. No error prose or input values cross this wire.

`get_pin_result(graphPath, output)` returns the current `ResultDescriptorDto` or null.
Descriptor/value/page commands take `{ executionSessionId, resultId }` references and read either current or leased results.
Descriptors carry this identity and immutable provenance; reading a retained descriptor does not change the current output index.
Run event identity uses `executionSessionId`, `graphPath`, and `runId`. `RunStarted { outputs }` invalidates current output bindings;
it does not revoke report leases. Failed or cancelled runs never expose an older successful payload as current.

`retain_result(reference, lease, handoff?)` atomically retains a snapshot and returns its descriptor.
`release_result_lease(lease)` is idempotent and validates the caller window; `reconcile_result_leases(leases)` releases
unlisted ordinary leases owned by that window. The client supplies a UUID token so an interrupted acquire can be cleaned up.
An optional handoff names the intended detached window; only that window may `claim_result_lease(lease)`.
Backend window destruction releases owned and pending handoff leases. Session replacement revokes all leases in that session.
These ownership commands delegate to Application/Execution; they never retain payloads in a second transport-side registry.

OLS report values contain an overview and typed table references, with no observation arrays or coefficient covariance matrix.
`get_result_table_page(reference, part, offset, limit)` accepts an execution-session-bound result reference and a fixed table part.
`analyze_result(reference, analysis)` accepts a tagged residual-plot, ACF/PACF, serial-test or hypothesis request;
clients supply query parameters only. Both commands dispatch Application work on a blocking worker and preserve its
session/result revalidation. Plot responses explicitly identify systematic sampling and population counts.
The result reference wire and report projections are owned by `schema/result.rs`; the computational model remains Rust-owned.

Project index, resource mutation, graph save and project save responses omit frontend undo status. Draft undo/redo belongs to the local draft; committed revisions, file transactions and failure recovery stay with the Project owner. Automation graph edit receipts carry draft revisions, graph hash, client correlation and created element identities, with no project undo capability or generated operation ID. These receipts describe unsaved draft edits. Graph Save returns the project instance, committed resource revision, document and projection replacement; its request operation ID stays within the commit path and is not echoed in the response.

## Frontend adapter

Plugin transport uses `command_plugin.rs` and the generic Rust Plugin Manager. Installation, enabled state, view attachment and task projections never depend on Julia availability. Julia/Bayes commands and DTOs are private to the external plugin; they are not Tauri commands. Generated plugin projections come from `yss-plugin-protocol` through `pnpm generate:plugins`. Plugin frames receive a scoped MessagePort, not Tauri access. The transport retains the exact common error wire; plugin failures expose only a stable `details.pluginCode` category.

View detach acknowledges completion and is idempotent for an already-released session. Cross-window or task-session detach is rejected. The page owner waits for that acknowledgement before replacing its lease. Concurrent activations share the Plugin Manager's bounded startup result; only lifecycle mutations remain `plugin_busy`. View quota failures use `plugin_view_limit`, and expired process instances cannot authorize old contexts. Frontend presentation retains the safe code and phase instead of collapsing every failure into a boolean.

Ordinary frontend invocation goes through `src/services/ipc/invokeCommand.ts`, which validates the common error wire. Domain services under `src/services/` own command-specific request/result parsing. Views and presentation modules do not call Tauri `invoke` directly.

Channel adapters parse strict wire DTOs before publishing to application projections. Malformed payloads and sequence gaps belong to each stream's recovery/status contract. Detection, UI visibility, and recovery are separate capabilities; their current coverage is documented by the stream owner, rather than guaranteed by DTO parsing alone.

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
