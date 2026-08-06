# Node Architecture Project Identity and Wire Closure Design

## Goal

Close the highest-risk remaining node-architecture gaps before continuing broader migration cleanup:

1. every active-project node command is owned by the caller's exact project lifecycle;
2. graph mutation direct results and project events carry one coherent project identity;
3. stale WebView commands have zero effects on a replacement project;
4. execution and project-event Rust/TypeScript wire contracts are strict and drift-resistant;
5. result-source capabilities cannot alias across project replacement;
6. every project database read and mutation is lifecycle-owned.

This is Batch A of the omissions recorded under `TODO.md` → `## 2026.08.06` → `node_architecture 遗漏审计`.

## Constraints

- Rust remains authoritative for project lifecycle, graph documents, history, compilation, execution, and publication revisions.
- React validates identity before any store or publication effect but does not infer backend ownership.
- Tauri commands remain thin: parse and validate input, call domain/application code, map DTOs, and emit events.
- `ProjectState.project_data` remains authoritative.
- Resource paths remain opaque.
- No compatibility envelope or optional identity field is introduced; this 0.x project uses one strict wire shape.
- Existing unrelated dirty work is preserved.
- Every behavior change follows RED-GREEN with focused tests before broader verification.
- Runtime scheduling, cache policy, relational adapters, legacy DTO cleanup, and projection refactors are outside this batch.

## Scope

### Active-project commands requiring explicit identity

The following commands accept a required `projectInstanceId` and validate it in Rust:

- `mutate_graph_document`;
- `update_function_signature`;
- `hydrate_editor_graph`;
- `get_project_history_status`;
- `undo_graph_document`;
- `redo_graph_document`;
- `execute_graph_document`.

The implementation includes a source/contract audit that forces every active-project command to declare its identity policy. This prevents future commands from silently bypassing lifecycle ownership.

### Project database commands requiring explicit identity

Every command that reads or mutates a database owned by the active project accepts a required `projectInstanceId`:

- lifecycle/resource mutation: `load_database`, `delete_database`, `rename_database`;
- reads: `get_database_meta`, `get_database_rows`, `get_column_stats`, `get_column_distribution`, `get_dataset_overview`, `get_edit_state`;
- edit mutations: `edit_cell`, `add_row`, `delete_rows`, `add_column`, `delete_column`, `cast_column`, `rename_column`, `undo_edit`, `redo_edit`, `save_database_changes`;
- snapshot export: `export_database`.

`list_sqlite_tables`, `list_sql_tables`, and `list_excel_sheets` inspect caller-selected external sources before project import and remain project-independent.

Rust validates reads against an identity-aware database snapshot and validates mutations again at their final filesystem/authority commit gate. Export snapshots data without holding project locks during external I/O, writes to a temporary output, revalidates lifecycle before publication, and atomically replaces the selected output only for the still-current project.

Database IDs remain project resource identifiers serialized under `databases/{database-id}`. They are not treated as globally non-aliasing capabilities.

### Explicit exclusions

The following remain unchanged:

- project activation and bootstrap commands, because they establish identity;
- global Registry construction and project-independent catalog assembly;
- follow-up operations authorized by process-global opaque `runId` or `sourceId` capability handles;
- project lifecycle workflows already governed by lifecycle mutation receipts.

`sourceId` remains a decimal-string opaque handle, but allocation is process-global and monotonic across every `ResultStore`. Creating or replacing a project never resets the allocator, so an old source handle cannot alias a replacement project's source. Follow-up source APIs remain unchanged.

## Authority model

### Frontend command ownership

An application workflow captures one immutable `ProjectIdentitySnapshot` before reading revision authority or invoking a command:

```text
{ projectInstanceId, epoch }
```

The workflow passes `projectInstanceId` to the service. After every asynchronous boundary it verifies that the snapshot is still current. A stale completion returns a stale/no-op outcome and produces no ordinary error toast.

Services only encode/decode IPC DTOs. They do not read Zustand, React context, or lifecycle globals.

### Rust command ownership

A command performs these steps:

1. parse the required `projectInstanceId`;
2. parse resource path, revision, operation identity, demand, or payload;
3. call a `ProjectState`/application method that accepts the expected project identity;
4. revalidate identity at the final authority gate before changing state;
5. return a project-scoped DTO;
6. emit a project-scoped event only after a successful commit.

Validation at command entry alone is insufficient. The final mutation, history, projection, or execution snapshot gate must reject a lifecycle replacement that occurs while the command is in flight.

### Stale command invariants

A stale command returns `stale_project_lifecycle` and must not:

- mutate `GraphDocument`, function signature, History, project data, or runtime graph state;
- allocate graph/resource/publication revisions;
- invalidate or publish compile products;
- insert a run into `ProjectRunRegistry`;
- expose or mutate a replacement project's result source or database through an aliased identifier;
- emit `GraphDelta`, `ResourceMutationCommitted`, or run events;
- modify frontend stores through a direct-result continuation, including stale pin-preview cleanup.

## Graph mutation result and event wire

### Direct result

`GraphMutationResultDto` includes required `projectInstanceId` alongside its existing delta, projection, and History data.

The identity is captured from the same committed authority state as the returned delta. It is not copied blindly from an unvalidated request.

### Event envelope

`GraphDelta` has one canonical payload:

```ts
interface GraphDeltaEventPayload {
  projectInstanceId: string;
  delta: GraphDeltaDto;
}
```

Rust serializes exactly these fields. TypeScript rejects missing or extra fields.

For one committed graph mutation, direct result and event have identical:

- `projectInstanceId`;
- `operationId`/`causedBy` correlation;
- graph path;
- `fromRevision` and `toRevision`;
- graph patch payload.

The event handler validates project identity before reading pending mutations, graph stores, catalog state, or publication state.

## Other command results

- Function-signature and History commands continue returning `ResourceMutationResultDto`, which already carries `projectInstanceId`.
- `hydrate_editor_graph` returns the existing projection DTO after Rust validates caller identity; the projection basis remains the response authority.
- `execute_graph_document` validates caller identity before capturing compilation/execution resources and before registering the run. `ExecuteGraphResultDto` remains `{ runId }`; the caller snapshot guards the direct completion, while run events retain their project/session correlation.
- `get_project_history_status` validates identity before reading the current History head.

## Frontend behavior

### Direct completions

Application coordinators use the captured identity snapshot around all command calls. If the lifecycle changes:

- the direct completion is ignored;
- no publication submission occurs;
- no store is updated;
- no ordinary failure notification is shown.

Pin-preview state is synchronously cleared by the project lifecycle reset. A stale preview event, rejection, or successful completion performs no Zustand read or write and cannot remove a same-path/same-port preview created by the replacement project.

### Project events

Raw backend events are parsed before dispatch. For project-scoped events:

1. validate exact wire shape;
2. validate required project identity;
3. compare against the current lifecycle;
4. only then perform deduplication, pending-operation lookup, recovery, or store effects.

A stale well-formed event is ignored. A malformed event for the current project is a protocol error and marks the affected projection stale when its graph identity can be safely extracted.

### Duplicate direct result and event

Existing operation-ID and fingerprint correlation remains authoritative. Matching direct/event delivery is idempotent. A conflicting payload for one operation/publication identity is a protocol error.

## Error and recovery behavior

### Stale lifecycle

`stale_project_lifecycle` is an expected concurrency outcome. It does not trigger project recovery because the response/event does not belong to the current project.

### Revision conflict

Revision conflicts retain their existing command-specific codes. They are distinct from lifecycle replacement.

### Revision gap

A current-project revision gap continues through authoritative snapshot recovery. This batch does not create a second recovery path.

### Event emission failure after commit

A committed direct result remains usable when event emission fails. The direct caller can settle publication; other windows recover through project index/publication revision reconciliation.

### Malformed wire

Strict parsers reject malformed current-project payloads with a protocol error. No partial DTO is committed to frontend state.

## Wire contracts

### Rust golden coverage

Canonical fixtures serialize all variants of:

- `GraphMutationResultDto`;
- `EventProject::GraphDelta`;
- `EventProject::ResourceMutationCommitted`;
- `ExecutionDemandDto`;
- `RunEventKindDto`;
- `ExecuteGraphResultDto`.

Fixtures use stable UUIDs, decimal-string opaque IDs, stable graph paths, and exact field allowlists.

### TypeScript contract coverage

TypeScript consumes the Rust-generated/checked-in fixtures through production parsers. Tests reject:

- missing required fields;
- extra fields on strict envelopes;
- unknown enum variants;
- unsafe numeric IDs where decimal strings are required;
- malformed graph resource paths and UUID-backed node/instance port identities;
- missing project identity;
- mismatched direct/event project or operation correlation.

The existing catalog/editor-projection golden contract remains intact.

## Implementation slices

### Slice 1: GraphDelta identity closure

- Add failing Rust serialization and emitter tests.
- Add `projectInstanceId` to the Rust graph mutation result/event authority.
- Update the TypeScript DTO/parser and handler.
- Add current-project, stale-project, malformed, duplicate, and conflict tests.

### Slice 2: Graph/function/projection command identity

- Add failing stale-before-entry and replacement-during-command tests.
- Thread required identity through services, commands, and final authority gates.
- Prove zero effects and zero events on rejection.

### Slice 3: History and execution identity

- Thread identity through History status/undo/redo and execution.
- Reject stale execution before run registration or channel events.
- Preserve current cancellation and result-source capability behavior.

### Slice 4: Golden contracts and architecture audits

- Freeze all listed execution and project-event variants.
- Add production TypeScript parsers.
- Add command identity-policy source audits.
- Run focused and broad verification.

### Slice 5: Final-review capability and database closure

- Move `sourceId` allocation to one process-global monotonic allocator while preserving the decimal-string wire and follow-up APIs.
- Add replacement and concurrency regressions proving old source handles cannot read, page, or release replacement-project sources.
- Thread required identity through every active-project database service and command.
- Validate database reads at snapshot and mutations at the final commit authority gate, with stale zero-effect tests.
- Move database commands out of capability exemptions in the architecture audit.

### Slice 6: Final-review parser and preview closure

- Reuse canonical graph-resource-path validation in execution demand, correlation, and event parsing.
- Validate UUID-backed `nodeId` and instance port `instanceId` fields.
- Make stale pin-preview settlement a complete no-op after lifecycle reset.
- Add malformed execution identity and same-key replacement-preview regressions.

## Test strategy

### Rust focused tests

- stale identity before command entry;
- project replacement after preparation but before final commit;
- graph/function/history rejection has zero authoritative effects and emits nothing;
- stale execution creates no run and emits no channel event;
- successful direct result and event are byte/field equivalent for identity and delta;
- exact event and execution serialization for every variant;
- duplicate operation behavior remains unchanged.

### Frontend focused tests

- services pass exact `projectInstanceId` arguments;
- application workflows capture one identity and reject stale completions;
- `GraphDeltaHandler` accepts current events and rejects stale events before store reads;
- raw event parsers reject malformed and extra fields;
- direct/event duplicates settle once;
- execution demand and run-event fixtures pass production parsers;
- malformed graph paths and UUID-backed port identities are rejected before invoke/callback effects;
- stale preview settlement cannot remove a replacement project's same-key preview;
- database services send exact required identity and stale reads/mutations have zero effects.

### Broader checks

For each slice, run the narrowest relevant tests first. Before delivery run:

- `pnpm typecheck`;
- relevant `pnpm test` files and the full frontend suite;
- focused Rust tests with serial execution where shared hooks/filesystem state require it;
- `pnpm rust:check`;
- `pnpm rust:fmt:check`;
- `git diff --check`;
- `pnpm verify` when environment permissions permit the documented Windows reparse-point tests.

## Acceptance criteria

Batch A is complete only when:

1. all listed active-project commands require and validate caller identity;
2. replacement-project races have automated zero-effect tests;
3. real Rust `GraphDelta` events are accepted by the current frontend and stale events are ignored;
4. direct graph mutation results and events carry coherent identity and delta correlation;
5. every listed execution/event DTO variant is covered by Rust↔TypeScript golden tests and strict production parsing;
6. no optional identity compatibility path remains;
7. process-global source IDs cannot alias across project replacement, and old handles cannot affect replacement sources;
8. every project database read and mutation requires and validates lifecycle identity;
9. malformed execution graph/port identities are rejected before effects;
10. stale pin-preview settlement performs no replacement-store effect;
11. focused verification and architecture audits pass;
12. `TODO.md` marks only the completed Batch A omissions as done, with fresh evidence.

## Out of scope

- frontend function-signature Pin synthesis removal;
- legacy node DTO cleanup;
- raw GraphDocument API restriction;
- frontend graph-reference cascade removal;
- exact resource-read version tracking;
- lowerability analysis refactoring;
- demand-result publication changes;
- runtime cache policy;
- relational materialization adapters;
- streams, backpressure, deadlines, retries, or parallel scheduling;
- Catalog search expansion;
- trace span redesign;
- `parameter_types` boundary movement;
- localization and History compatibility cleanup.
