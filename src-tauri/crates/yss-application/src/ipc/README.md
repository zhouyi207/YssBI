# Application IPC transport contract

> Status: Current
> Scope: Tauri commands、events、channels、wire DTOs and transport errors
> Canonical owners: Application's IPC module and the Event/Channel/Contract crates own executable wire facts; this README owns the shared transport contract
> Update when: public transport shapes、registry ownership、delivery semantics or frontend invoke boundaries change

YssBI's desktop IPC boundary has an Application-owned command module and three supporting crates:

| Owner                                                     | Responsibility                                                                                                                                   |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `yss-application::ipc`                                    | Application commands and invoke registry, request/response mapping and diagnosed errors; application execution and graph-client channel adapters |
| [`yss-ipc-event`](../../../yss-ipc-event/README.md)       | Event delivery after authoritative commits                                                                                                       |
| [`yss-ipc-channel`](../../../yss-ipc-channel/README.md)   | Neutral Harness event subscriptions and project progress delivery                                                                                |
| [`yss-ipc-contract`](../../../yss-ipc-contract/README.md) | Shared wire DTOs, event envelopes and the exact error payload                                                                                    |

Event and Channel depend on Contract and never depend on Application. Contract has no Tauri or runtime dependency. Execution encoding and graph-client handoff consume Application types, so they live in [channel/](channel/mod.rs) within this module. Business workflows and committed state remain in their use-case/domain owners.

## Public surface

Application exposes `invoke_handler()` from [mod.rs](mod.rs) alongside `initialize(app)`. The desktop entry connects these two functions directly to Tauri. Commands, schemas, response caches and diagnosed errors remain private to the IPC module.

Application initialization directly constructs the concrete `CommandRuntime`, obtains its Harness ports, builds the business services, and installs the command contexts using the same channel hubs. There is no separate Command crate, Application startup plugin or binding registry. Platform plugins own their namespaced command registries.

`tauri-plugin-tracing` owns `plugin:tracing|...` log commands, SQLite history and log Channels. Structured runtime observations are submitted through Rust tracing or the frontend LogService. Application has no separate operational-diagnostic command registry or stream. See [Runtime Signals](../../../../../docs/architecture/RUNTIME_SIGNALS.md).

`HarnessRuntimeState` holds the Host/provider and the shared hubs. `ActivityPanelSyncState` remains a response cache; `ApplicationCapabilityGateway` schedules internal Application capabilities and read-only graph receipt recovery. Graph activity is a separate projection channel. Logging, project state, samples, watchers and Plugin Manager remain owned by their existing runtime services.

Native window geometry uses the official Window State plugin registered by the composition root.
There are no YssBI window-state query/save commands or geometry DTOs. The frontend creates hidden
windows through the platform adapter and the Rust plugin owns restoration and persistence;
see [Workbench window geometry](../../../../../docs/architecture/WORKBENCH_LAYOUT_ARCHITECTURE.md#81-原生窗口几何与关闭).

## Command responsibilities

A command handler may:

1. parse and validate IPC input;
2. convert wire DTOs to application/domain types;
3. call one application/domain use case;
4. move blocking work to the established blocking boundary when needed;
5. map typed outcomes to wire DTOs or `CommandError`;
6. deliver an event/channel message after the authoritative operation reaches its documented commit point.

A command handler may not own filesystem transactions, long workflows, duplicated domain validation, Project/Graph reconciliation, statistical computation, or durable state. A delivery failure can produce a transport failure, but it does not pretend that an already committed authority mutation never occurred.

Flat revision-checked commands retain their required wire fields. Where Tauri's
injected app/state parameters push a handler over Clippy's argument threshold,
the handler records a local lint expectation with that reason. Internal use cases
still remove redundant inputs. Boxing the project event payload only changes its
in-memory representation; the tagged event wire remains unchanged.

Standalone statistical commands convert DTOs and directly call the stateless
`yss-sci-runtime` functions. ACF/PACF retains Application session admission and its
60-second deadline. Analyses of retained graph results go through Application's
session/result validation and Execution's result analysis functions. Only
`yss-application::ipc` and `yss-graph-execution` directly consume SCI runtime; numerical rules
remain in `yss-sci` and shared data/control types in `yss-sci-contract`.

`ApplicationCapabilityGateway` is the injected scheduling adapter for the internal Assistant capability port. It moves the synchronous Application use case to the blocking pool, enforces the supplied read-only deadline/cancellation budget, and maps worker failures to typed capability failures. Harness continues to own tool admission, ledger, lifecycle events, and turn state; it never calls Tauri commands as its business bus.

Graph tools call Application directly through the capability gateway. Application reads the Project-owned editing state and validates the current revision/hash before editing, validating, running or saving. Rust returns the capability result; Graph Activity carries editing and execution notifications to frontend consumers.

Assistant `apply_graph_edit` atomically persists its complete current graph through the Project file transaction, retaining undo history. It needs no editor panel or Webview acknowledgement. A successful receipt confirms both the edit and persistence; a file failure leaves the prior graph state intact and returns `persistence_unavailable`. GUI edits, validation and execution keep their existing explicit-save behavior.

`update_function_signature` accepts `projectInstanceId`, `functionPath` and the
revisioned `request`. The mutation is locale-independent; localized projection
refreshes supply their locale through the projection query.

## DTO ownership

The `semanticDomain` parameter editor carries an object with `values: [{ value, label }]`
and optional `positiveValue`. Codes are exact strings and ordered rows define ordinal levels.
Rust validates the domain; the frontend only owns the editing draft. Annotated runtime values
retain conversion metadata internally, while result JSON and pagination project their underlying
scalar/list payloads through the existing result contract.

Node catalog items and Activity node rows carry a required boolean `available`, computed
from the current session's frozen kernel registry. Desktop catalogs preserve unavailable
items and their category hierarchy; clients display them disabled and prevent creation
through drag, click or keyboard selection. AI catalog search continues to return only
available items. Creation descriptors retain their existing shape.

Harness tool start/completion/failure events carry the same invocation ID. Failure events expose only the stable `failureCode`, including cancellation and timeout. Harness channel subscriptions buffer live events until historical replay has been merged, then deliver each sequence once. The frontend consumes this stream as a projection.

Wire DTOs are explicit transport types. Internal structs are not exposed merely because they implement serialization. Mapping is owned at this seam; domain/application crates do not depend on Tauri or frontend wire schema.

Use stable camelCase fields and strict frontend parsers for public DTOs. A contract change updates the Rust mapper, TypeScript parser/types, representative boundary tests, and the owning architecture document together. YssBI 0.x contracts are migrated directly unless compatibility is an explicit product requirement.

Graph documents carry an optional `constants` map keyed by stable UUID. Each constant contains `id`, `name`, `dataType`, Rust-tagged `dataValue`, and optional tabular snapshot, description and tags. `setConstant { id, constant }` replaces a definition or deletes it with `null`; `insertConstantReference { id, position }` inserts its Get node. These are Graph draft mutations and use the existing Transform, Resolve, Save and history contracts. Node parameter editor `graphConstant` selects a definition in the current graph. Clipboard snapshots optionally carry referenced constant definitions in `constants`.

Graph value types use `ValueType`: `{ kind: "Scalar", inner: "Numeric" }` references the shared
seven-way Semantic contract; `DataSeries.inner` contains the element value type and DataFrame
retains its independent structure. Schema field scalar types are Semantic names or null when
unresolved. Physical integer/float/boolean/string tags remain in `dataValue`, not in graph type
declarations. Changing only a constant's Semantic preserves its physical value and embedded
snapshot; incompatible choices are rejected by Rust. Project files and live IPC use the current
type contract directly, without migration or compatibility conversion for legacy declarations.

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
Rows contain the original physical values, using the existing exact display encoding. Semantic
maps, ordinal ranks and binary event encodings are internal interpretations and never replace
the values or physical type labels shown in DataView.

Database column projections carry `name`, the existing `type` display label, exact `physical`,
and `semantic: { kind, values, positiveValue, numeric }`. Semantic is one of the seven field
meanings specified by the [dataset contract](../../../yss-database-store/README.md#field-meaning-and-physical-conversion).
Codes and numeric bounds are strings, preserving wide integers and decimal precision. The
`set_column_semantic` mutation takes `projectInstanceId`, `operationId`, `expectedRevision`,
`id`, `colName` and that Semantic configuration. It returns the existing database mutation
aggregate; no data values or Physical changes accompany the semantic edit. `cast_column`
converts Physical while retaining and validating Semantic. Both commands perform scanning and
conversion on the blocking pool, then publish the normal revision-checked resource changes.

`get_result_page` 在 blocking worker 上调用 Application 的页面查询；命令不直接执行关系查询或持有数据库锁。
`ResultPage.totalCount` 为可空整数，未知总数使用 null；`hasMore` 与 `nextOffset` 决定是否可继续翻页。
关系页面的 metadata 为 `{ columns: [{ name, type }] }`，values 为按该列顺序排列的行数组。
超出 JavaScript safe-integer 范围的单元格使用十进制文本，精确存储类型保留在列元数据中。
日期时间单元格使用不带时区的日历/钟面字符串，类型名只包含时间单位；其转换语义由
[Dataset store](../../../yss-database-store/README.md) 维护。

Choose the transport by semantics:

| Primitive      | Use                                                                                |
| -------------- | ---------------------------------------------------------------------------------- |
| Command        | bounded request/response work with one typed outcome                               |
| Event          | low-rate state-change notification that does not carry authority                   |
| Channel/worker | ordered, streaming, high-frequency, progress, execution, logs, or Harness delivery |

Every ordered stream defines its source identity, ordering key, capacity/backpressure behavior, loss/gap handling, replay or snapshot recovery, cancellation, and terminal semantics in its domain owner. `yss-ipc-channel` maps application contracts to Tauri without inventing a second queue model. Application subscriptions and cancellation remain in this registry; plugin logs use the plugin registry.

Events are notifications, not state stores. Consumers recover authoritative data through the domain’s snapshot/query command rather than rebuilding it from an assumed complete event history.

Graph editor projections arrive in load/hydrate/mutation/Resolve/Save command responses. Execution prepares plans internally. Graph Activity notifies consumers when editing or execution changes; Logs and Execution retain their own contracts. Log delivery and failure semantics are documented in [Runtime Signals](../../../../../docs/architecture/RUNTIME_SIGNALS.md#3-logging).

Project registration commands consume Application's process-wide `ProjectManagement`. Application owns registry construction from the injected store and picker-task cancellation admission/cleanup. Commands retain progress-channel binding, draining and wire/error mapping. Project activation and replacement continue to use the existing Application session slot; registry state is not replaced with that session.

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
See [Workbench](../../../../../docs/architecture/WORKBENCH_LAYOUT_ARCHITECTURE.md) for UI ownership.

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
[Graph and Execution](../../../../../docs/architecture/GRAPH_AND_EXECUTION.md#save).

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
- `incidentId` is always present and is `null` unless technical-log correlation is required.

The wire never contains a backend-owned `message`. Do not encode identity in string prefixes, return `Result<T, String>` from a command, or accept legacy error shapes. Expected failures map to stable code/details. Internal or infrastructure failures generate an incident identity and write technical context only through sanitized tracing logs.

Successful DTOs and asynchronous statuses may not bypass this rule with backend prose fields such as `message`, `detail`, `hint`, or `reason`. Domain diagnostics that are intentionally user-visible use a stable code, safe location/parameters, and their domain-owned deterministic template contract.

Editor diagnostics carry code, messageKey, safe arguments, severity, explicit blocking, location and related locations. Rust owns definitions/templates; React uses the generated vocabulary for localization. This domain fact contract remains separate from CommandError.

The read-only `resolve_editor_graph` command supplies current projections for refresh, including execution capability diagnostics. `execute_graph` accepts the current editing `version`, `semanticInputHash` and explicit demand, reads the matching document and prepares its plan internally. A stale semantic identity returns `graph_draft_changed`; blocking diagnostics return `graph_not_ready`. Internal resolution/plan failures retain diagnostic incidents. Plan identities and caches stay inside Execution. See [Graph and Execution](../../../../../docs/architecture/GRAPH_AND_EXECUTION.md).

Frontend application code localizes `code + safe details`; `IpcError.message` is a technical summary and must not be rendered directly.

Harness event types use snake_case while envelope and payload fields use camelCase. Provider turn failures retain stable categories for authentication, rate limits, rejected requests, connection failures, unavailable services and invalid responses. The error wire never includes a raw provider response or credential. See [Statistical Harness](../../../../../docs/architecture/STATISTICAL_HARNESS.md).

Execution channels deliver `RunEventDto` directly for lifecycle and result notifications. Analysis Graph has no stdout/stderr message or callback. Terminal failures carry `RunErrored { code, phase, source }`, where source is null or a safe graph/node/port identity. Numeric failures retain their specific cause. When a rejected execution command has already delivered a terminal event, its details include `terminalRunEventSent: true`; the frontend drains the channel before finalizing the command failure so the Output panel retains the typed cause. No error prose or input values cross this wire. Harness events and plugin process communication retain their own protocols.

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

Project owns the current graph document, reversible history and saved-content fingerprint. Graph commands receive an editing version, not a client-authored document. Replies carry Rust-owned dirty/canUndo/canRedo facts. Save persists the matching current document and clears history only on success. Harness graph receipts retain their client correlation and created element identities; internal Project operation IDs are not exposed as a second history API.

## Frontend adapter

Plugin transport uses `command_plugin.rs` and the generic Rust Plugin Manager. Installation, enabled state, view attachment and task projections never depend on Julia availability. Julia/Bayes commands and DTOs are private to the external plugin; they are not Tauri commands. Generated plugin projections come from `yss-plugin-protocol` through `pnpm generate:plugins`. Plugin frames receive a scoped MessagePort, not Tauri access. The transport retains the exact common error wire; plugin failures expose only a stable `details.pluginCode` category.

View detach acknowledges completion and is idempotent for an already-released session. Cross-window or task-session detach is rejected. The page owner waits for that acknowledgement before replacing its lease. Concurrent activations share the Plugin Manager's bounded startup result; only lifecycle mutations remain `plugin_busy`. View quota failures use `plugin_view_limit`, and expired process instances cannot authorize old contexts. Frontend presentation retains the safe code and phase instead of collapsing every failure into a boolean.

Ordinary frontend invocation goes through `src/services/ipc/invokeCommand.ts`, which validates the common error wire. Domain services under `src/services/` own command-specific request/result parsing. Views and presentation modules do not call Tauri `invoke` directly.

Channel adapters parse strict wire DTOs before publishing to application projections. Malformed payloads and sequence gaps belong to each stream's recovery/status contract. Detection, UI visibility, and recovery are separate capabilities; their current coverage is documented by the stream owner, rather than guaranteed by DTO parsing alone.

## Data and security boundary

- Keep large datasets and computation in Rust; expose paging, projection, batching, handles, or result IDs.
- Do not send raw infrastructure errors, SQL, connection strings, credentials, prompts, transcripts, document content, clipboard content, or table rows through error details or log fields.
- Treat resource paths and IDs as opaque values; do not normalize domain identity in transport/UI code.
- Credential configuration uses explicit injected/application paths and is never persisted in Harness/Project/logging by this crate.

## Related owners

- [System architecture](../../../../../docs/architecture/ARCHITECTURE.md)
- [Graph and Execution](../../../../../docs/architecture/GRAPH_AND_EXECUTION.md)
- [Runtime Signals](../../../../../docs/architecture/RUNTIME_SIGNALS.md)
- [Statistical Harness](../../../../../docs/architecture/STATISTICAL_HARNESS.md)
- [Change Process](../../../../../docs/development/CHANGE_PROCESS.md)
- [Local Workflow](../../../../../docs/development/LOCAL_WORKFLOW.md)

The exact command list is executable source in `mod.rs` and must not be copied into this README.

## Graph projection synchronization

Graph load/hydrate/edit/history/resolve/save replies use GraphEditorSyncResponseDto. Its immutable session payload contains document, projection and editing state; the envelope retains command outcome and optional save metadata. GraphEditorSyncState binds bounded baselines to window/project/graph/locale and editing session. Set/remove paths are read-projection delivery operations, independent of GraphDocumentPatch business edits.

Initial reads, missing baselines and larger deltas use snapshots. Oversized cache entries remain readable as uncached snapshots. The client preserves untouched references, validates the complete candidate and uses one read-only hydration recovery after a bad delta; it never repeats a write to repair presentation. GraphActivity notifications are coalesced with GUI replies. Lag recovery reads current graph snapshots and known run states from Execution; detailed lost run events are not reconstructed from graph state.

Snapshot and delta frames carry `snapshotBytes`, the Rust-computed serialized size of the complete session. Both transport caches enforce entry and byte limits; the frontend uses this count instead of serializing the full graph on the main thread. Hydration includes the current function editor projection so a recovered function save has the same contract as a normal reply.

`get_graph_edit_receipt` is a read-only, project/graph/editing-version/operation-bound query. It waits behind that graph's active operation and returns the original committed version, command kind and changed flag, or null when no receipt is retained. Null is not proof of rollback. On a missing/malformed command reply, the client queries once and hydrates the latest state only after confirming the original commit. A known business rejection is not converted into success. Save recovery retains the original saved revision and the latest backend dirty/history facts. Released frontend epochs and expired backend editing sessions cannot install a recovered projection.

Graph write and projection encoding work runs on blocking workers. Window destruction, frontend cleanup and project session replacement release graph activity channels. The retired Harness graph-client prepare/claim/adopt commands and wire models are removed.
