# Shared desktop IPC contract

> Status: Current
> Scope: Shared wire DTOs, event envelopes and command error payloads
> Canonical owners: This crate owns shared serialization; adapters own conversion from application/runtime facts
> Update when: A shared IPC payload changes

Editor schema summaries include the `fixed` kind for Catalog-declared output
fields. Their names and scalar types use the existing resolved-field payload;
clients do not maintain node-specific result-table definitions.

`yss-ipc-contract` defines project receipts, graph projections, Harness messages, execution stream values, project progress and `CommandErrorDto`. Command, Event and Channel consume these same types.

Node editor capabilities contain only the required `managed` boolean. Copy, duplicate, delete and cut availability derive from this ownership flag; the node capability payload rejects missing or unknown fields. Parameter and inline-literal editors consume their own existing projections.

Node parameter groups have `{ key, display, parameters }`. Every parameter carries an
`editor` object discriminated by `kind`; plain controls and schema-aware column/filter controls
share this field. A `select` editor contains `options`, with null retaining text entry for
text-valued parameters without projected choices. Parameter values, display metadata, presentation
and multiline behavior retain their existing meaning. The desktop accepts only this shape.

Desktop port connection capabilities contain `current`, `canAppend`, `canReplace` and `canMove`.
Accepted type domains, maximum capacity and ordering stay with backend owners; the desktop uses
connection candidate decisions for legality and replacement previews. Harness keeps its consumed
accepted-type description and maximum-connection facts through its own contract.

Graph editor sessions require `document`, `projection`, `editing`, and `resultState`. The result summary carries the matching semantic input hash, execution session ID and a decimal-string `revision` independent of the edit revision. Snapshot and delta delivery carry the same complete contract. Clients atomically publish graph entities, editing metadata and result validity, and reject older result summaries for the same execution session and semantic identity.

The crate has no Tauri, Application, database or execution runtime dependency. It reuses neutral identity/document contracts and existing projection value types, including their value conversions. It creates no subscriptions, tasks, caches, incident records or business state. Application mappings and incident recording remain in their adapters.

Execution demands use `default` or `outputs`; run events describe run lifecycle and result inspection requests. Pin View reads current results through result queries and has no separate execution demand, generation allocator or completion event.
Every run event carries `{ executionSessionId, graphPath, runId, semanticInputHash }`.
The required hash is the 64-character lowercase hexadecimal encoding of the semantic basis
captured and validated for that run. Invocation channels, public notifications and recovery
snapshots use the same identity; clients must not derive it from their current editor state.
The event envelope also requires `resultRevision`, an unsigned 64-bit decimal string captured
from ResultStore when that event was produced. It is independent of RunIdentity and may advance
between start and completion. A matching graph result summary covers the event only when its
revision is at least this value; recovery retains the original event revision.
An `outputs` demand includes `includeDefaultResults` and the required boolean `reuseInputs`. Report extensions set `reuseInputs` to true to reuse valid upstream inputs while recomputing requested outputs; ordinary output runs set it to false. Execution owns cache validation and publication checks.

Command-only request/response schemas may stay beside their handler. Shared types have one definition here, with no compatibility re-export from the former schema modules.

Harness tool lifecycle events are `tool_invocation_started`, `tool_invocation_completed`, and `tool_invocation_failed`. Each carries its actual invocation identity and capability ID; there is no pre-identity placeholder event or legacy event conversion.

`graph_execution_finished` reports the graph's business outcome independently of
tool-call completion. Agent completion retains its exact failure code. Resume,
runtime recovery, text retraction, compaction and blocked delivery are replayable
events; compaction exposes its occurrence without sending model checkpoint text to the UI.
`context_compaction_progress` carries `completedBytes` and `totalBytes` for both
Manager and Worker progress. Durable partial checkpoint text and prefix hashes
remain inside the Harness; they are excluded from the wire projection.

Harness workflow state reaches desktop consumers through `HarnessEventDto`. Session and turn
command responses have their own DTOs; workflow run records remain in the Harness contract.

Agent completion includes committed resource changes and result references for presentation.
Result IDs in this UI projection are decimal strings, preserving the full u64 identity.
`HarnessToolInspectionDto` is a read projection of the existing ledger: it contains selected
operation facts, target identity, artifacts/results and recorded timing, excluding raw request
payloads, credentials, rows and document bodies. It does not alter model messages or stored receipts.

The complete wire and delivery contract is maintained in [Desktop IPC](../yss-application/src/ipc/README.md).

Harness model configuration uses the provider-neutral model contracts from `yss-harness-contract`.
`SaveHarnessProviderRequestDto.apiKey` is input-only: null retains the saved key, empty text clears it,
and a nonempty key replaces it in the OS credential store. It has no Debug/Serialize implementation.
`DiscoverHarnessModelsRequestDto` carries an unsaved provider connection in `config` and an
input-only optional `apiKey`. Model definitions are ignored during discovery. A supplied key is
temporary; null reuses the key saved for `config.id`, and no-key authentication ignores credentials.
The request has no Debug/Serialize implementation and never commits settings or credentials.
Provider configuration keeps the supplier name in `name` and a separate nullable `customName`
for an optional configuration label. Provider list titles, configuration breadcrumbs and model-picker
prefixes use the trimmed `customName`, falling back to `name` when it is blank. Provider lists still
show the actual supplier `name` before the model count below the title. Both fields round-trip through
save and catalogue responses. A turn's model identity captures that configuration display name in
`providerName` at admission; replay uses the recorded name even after the configuration is renamed
or deleted. Provider/model IDs remain the selection identity.
The catalogue returns `hasApiKey`; sessions carry nullable `model` selection. Every `turn_started`
payload records the executed model identity, display names, and `resources: [{resource: {kind,id}, name}]`
for history replay. Input references carry only project resource identities; Application supplies
names after resolving the current project index. The former implicit active-graph argument is removed.
The frontend
strictly parses these current shapes; obsolete provider-status and global-driver configuration
commands are removed.

Harness 引用详情返回 `{ text, resource }`：`text` 仅为已核验的引用片段，`resource` 是可打开的项目文档定位或 Null（内置来源）。项目知识来源管理返回来源 ID、标题、路径、状态与更新时间；内部项目根身份和内容摘要不进入此管理投影。当前状态为 `ready`、`changed`、`unavailable` 或 `empty`，业务规则归 [Harness 当前架构](../yss-harness-core/README.md)。
