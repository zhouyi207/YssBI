# Unified Result Store Design

Date: 2026-08-13
Status: Approved design

## Summary

YssBI will use one project-session-level `ResultStore` as the authoritative
owner of execution results. A kernel activation consumes its complete input
set, runs once, computes its complete output set, and atomically publishes the
outputs as independently addressable results. Each output data pin receives a
`ResultId`; the activation, output Pin history, downstream nodes, result
windows, reports, plots, inspectors, and memoization all refer to that same
identity.

The design removes the separate replayable snapshot path. View Data will no
longer copy or rematerialize its input and will no longer expose a `snapshot`
pin. It will open the exact `ResultId` supplied to its Data input.

Large values remain physically stored once. Shared in-memory and spill-backed
storage are private `StoredValue` implementations rather than separately
addressable artifacts.

## Goals

- Make `ResultStore` the sole business owner of execution result state and
  values.
- Give every output data pin result a stable `ResultId` before execution
  completes.
- Execute a kernel once per activation using all required inputs and compute all
  declared outputs together.
- Atomically publish an activation's complete output set.
- Preserve every pending, successful, failed, and cancelled output occurrence
  for the lifetime of the current project session.
- Store large data once while allowing multiple independent readers.
- Make downstream execution and all presentation surfaces use the same result.
- Remove View Data snapshots, rematerialization, replayable artifacts, and
  presentation inference through special lineage traversal.
- Preserve streaming, spill, backpressure, cancellation, and materialization
  budget capabilities without retaining a second business-level storage model.

## Non-goals

- Persisting execution history across project close or project switch.
- Automatically evicting results during the current project session.
- Allowing ordinary downstream kernels to consume partially written results.
- Adding partial-success semantics for individual outputs of a normal kernel
  activation.
- Caching duplicate values on input pins.
- Treating a physical spill resource identifier as a domain result identity.

## Core invariants

1. A value produced for an output data pin has exactly one `ResultId`.
2. Each output data pin in an activation has an independent `ResultId`.
3. All output data pins of an activation form one atomic result group.
4. A normal kernel receives all required inputs and runs exactly once per
   activation to compute all outputs.
5. The output group moves atomically from `Pending` to one terminal outcome.
6. Terminal states are immutable.
7. Pin history, Frames, windows, and memoization hold references, not copies of
   the result value.
8. Input pins do not own result histories or duplicate upstream values.
9. Mapping between kernel outputs and pins follows protocol `port_sequence`;
   map or alphabetical ordering is never part of the kernel ABI.
10. Results remain available until explicit clearing or the current project
    session ends.

## Domain model

### Result store

Each loaded project session owns one authoritative store:

```rust
struct ResultStore {
    results: HashMap<ResultId, StoredResult>,
    pin_history: HashMap<OutputPinId, Vec<PinResultEntry>>,
    groups: HashMap<ActivationId, ActivationResultGroup>,
}
```

The store is responsible for:

- allocating result identities;
- creating complete pending activation groups;
- managing result state transitions;
- owning successful result values;
- retaining provenance and presentation metadata;
- indexing all output-pin occurrences for the session;
- serving result queries and subscriptions;
- releasing all results when the project session ends.

The store does not schedule nodes, own execution-plan variables, open windows,
or persist data into project files.

### Stored result and state

```rust
struct StoredResult {
    id: ResultId,
    state: ResultState,
    provenance: ResultProvenance,
    presentation: ResultPresentation,
}

enum ResultState {
    Pending(ResultProgress),
    Ready(StoredValue),
    Failed(Arc<ResultFailure>),
    Cancelled,
}
```

Allowed transitions are:

```text
Pending -> Ready
Pending -> Failed
Pending -> Cancelled
```

A rerun creates new result identities rather than reopening terminal results.

### Provenance

```rust
struct ResultProvenance {
    run_id: RunId,
    activation_id: ActivationId,
    graph_path: GraphPath,
    graph_revision: GraphRevision,
    node_id: NodeId,
    output_pin_id: Option<OutputPinId>,
    created_at: Timestamp,
}
```

Public node outputs have an `output_pin_id`. Compiler-inserted adapters and
materialization steps may produce internal results without public pins. Such
results still have `ResultId`, provenance, state, and presentation, but do not
appear in public Pin history.

### Physical value storage

```rust
enum StoredValue {
    Scalar(Value),
    InMemory(Arc<[Value]>),
    SpillBacked(Arc<SpillStorage>),
}
```

The exact variants may follow existing canonical value types, but the boundary
is fixed:

- `ResultId` is the only business data identity;
- shared references do not duplicate underlying values;
- spill-backed storage can open multiple independent readers;
- spill file handles and internal resource identifiers never appear in Frame,
  Pin, window, IPC, or result-lifecycle models;
- "replayable artifact" is removed as a business concept.

### Activation result group

```rust
struct ActivationResultGroup {
    activation_id: ActivationId,
    output_result_ids: Vec<ResultId>,
}
```

`output_result_ids` follows output `port_sequence`. The group identifies the
atomic commit boundary; it does not own a second copy of result data.

### Pin result history

```rust
struct PinResultEntry {
    result_id: ResultId,
    run_id: RunId,
    activation_id: ActivationId,
    graph_revision: GraphRevision,
    created_at: Timestamp,
    usage: ResultUsage,
}

enum ResultUsage {
    Produced,
    Reused { original_activation_id: ActivationId },
}
```

The authoritative lookup is:

```text
OutputPinId -> PinResultEntry -> ResultId -> StoredResult
```

History rules:

- append an entry when the activation's pending group is created;
- retain pending, ready, failed, and cancelled occurrences;
- never silently replace or fall back to an older successful result;
- retain all entries for the current project session;
- release all entries and results when the project closes or switches;
- a future explicit clear operation may remove history, but automatic eviction
  is outside this design.

Duplicated query metadata in `PinResultEntry` is an index optimization only.
State, errors, presentation, and values remain authoritative in `StoredResult`.

### Execution Frame

```rust
struct Frame {
    bindings: HashMap<ValueRef, ResultId>,
    // Existing control-flow and activation-local state remains here.
}
```

A Frame is the execution plan's short-lived symbol table. It records the exact
result selected for an activation-local `ValueRef`. It does not own complete
values, artifacts, snapshots, Pin history, or result lifetime.

Frames remain necessary because plan-local adapter outputs, control flow,
loops, concurrent activations, and internal values cannot be represented by
reading the latest value of a public Pin. Destroying a Frame removes only its
bindings; it does not remove referenced results.

## Kernel activation semantics

A normal kernel is conceptually:

```text
Kernel(all inputs) -> all outputs
```

For a node with outputs `result` and `report`, execution is not:

```text
calculate result -> publish result -> calculate report -> publish report
```

It is:

```text
resolve all inputs
  -> run kernel once
  -> obtain [result, report]
  -> validate complete output set
  -> prepare complete StoredValue set
  -> atomically publish the activation group
```

Each output still has a separate `ResultId` because outputs can have different
connections, types, presentation modes, Pin histories, and viewer lifecycles.
Computation and publication are activation-level operations; identity and
querying are per output.

Kernel code does not allocate `ResultId`, update Pin history, own persistent
values, publish result sources, or open UI windows.

## Execution data flow

### Pending group creation

Before invoking a kernel, the scheduler asks `ResultStore` to create the entire
pending output group in one operation. The operation:

1. validates every expected data output descriptor;
2. allocates one `ResultId` per output in `port_sequence`;
3. inserts all `StoredResult::Pending` records;
4. records the activation group;
5. appends entries for public output pins;
6. returns bindings used to update the Frame.

If setup fails, no partial group or partial Pin history is retained.

### Input resolution

Inputs resolve through current activation bindings:

```text
Input ValueRef
  -> Frame binding
  -> upstream ResultId
  -> ResultStore
  -> StoredResult
```

The scheduler never substitutes a public Pin's latest historical result for the
current activation binding.

Input state handling:

- `Ready`: expose a read-only value accessor to the kernel;
- `Pending`: wait for the exact dependency result;
- `Failed`: do not invoke the kernel; fail its pending output group with an
  upstream-cause chain;
- `Cancelled`: do not invoke the kernel; cancel its pending output group;
- missing `ResultId`: report an internal consistency failure.

### Kernel invocation and output preparation

The kernel receives all required ready inputs in input `port_sequence` and is
invoked once. It returns the complete output collection in output
`port_sequence`.

Before acquiring the store lock, execution infrastructure:

- validates the output count;
- validates descriptors and types;
- materializes or spills each output as required;
- constructs the complete `StoredValue` collection.

Potentially blocking work, I/O, and large materialization never run while
holding a global store lock.

### Atomic group completion

`ResultStore::complete_group` takes a short lock and verifies that:

- every group result still exists;
- every group result is still `Pending`;
- the output count and fixed order match;
- all results belong to the same activation.

Only after all checks pass does it transition the entire group to `Ready`. No
observer or downstream node may see a partially ready normal activation.
Notifications are emitted after releasing the lock.

## Streaming and pending writers

Streaming remains an execution and physical-storage capability, not a separate
result model. A streaming kernel still represents one activation that computes
its complete declared output set.

A pending writer is bound to an already allocated output `ResultId`:

```rust
struct PendingValueWriter {
    result_id: ResultId,
    storage_builder: StorageBuilder,
}
```

The builder can accumulate in memory or spill to private temporary storage.
Writers do not allocate a second queryable identity. All writers must finish
and yield complete `StoredValue`s before the group can atomically become
`Ready`.

`Pending` exposes stable status and progress only:

```rust
struct ResultProgress {
    phase: ProgressPhase,
    completed_units: Option<u64>,
    total_units: Option<u64>,
}
```

A window may show queued, running, materializing, spilling, or committing
progress, but renderers and ordinary downstream kernels cannot consume partial
business data. True producer-consumer streaming would require a separate,
explicit execution protocol and is not implied by ordinary data pins.

## Failure and cancellation

### Kernel failure

If a kernel fails before the group commits, all outputs in that pending group
atomically become `Failed`. Results may share an `Arc<ResultFailure>` to avoid
copying diagnostics. Previous successful activations remain unchanged.

### Upstream failure

When a required input is failed, the current kernel is not called. Its output
group becomes failed with cause metadata containing the upstream `ResultId` or
all failed input result IDs with a deterministic primary cause. It never falls
back to an older successful Pin result.

### Cancellation

Cancellation applies to the activation group:

1. signal the kernel and pending writers;
2. stop new writes;
3. clean uncommitted temporary storage;
4. atomically move every pending group result to `Cancelled`;
5. retain result and Pin-history metadata.

Concurrent completion and cancellation use state checks so exactly one
terminal transition wins. Terminal results are never reopened.

### Group events

After a terminal group transition, the store publishes one activation-level
change event containing the group result IDs. The scheduler wakes dependencies
only after the whole group is terminal. Pin projections and windows observe the
same event and the same results; they do not own data.

## Memoization

Memoization changes from cached runtime values to cached result references:

```rust
struct MemoizedActivation {
    output_result_ids: Vec<ResultId>,
}
```

On a hit:

- the kernel does not rerun;
- the Frame binds to existing `ResultId`s;
- values are not copied;
- current Pin histories append `Reused` usage entries;
- original producer provenance remains on the stored results;
- the usage entry records the current activation and original activation.

Memoization is scoped to the same project session as the store and may not hold
result references after session teardown.

## View Data

The final View Data interface is:

```text
Enter -> View Data -> Then
Data  ->
```

View Data is an execution side effect, not a data transformation. It:

1. obtains the Data input's exact `ResultId` from the current Frame binding;
2. emits an application request to open or focus a result window for that ID;
3. writes an appropriate `NOTIFY` log entry;
4. continues through `Then`.

It does not read, copy, materialize, spill, transform, or republish the value.
It does not choose a renderer. `StoredResult.presentation` determines the
presentation.

The following are removed:

- public `snapshot` output pin;
- View Data `FragmentResult` and `result_leaf` behavior;
- View Data cursor replay and rematerialization;
- construction of `ArtifactKind::Replayable`;
- special default-result publication for View Data;
- special View Data or adapter lineage traversal to recover presentation.

For `OLS Summary.report -> View Data.data`, the report Pin, Frame input, View
Data event, and report window must all contain exactly the same `ResultId`.

## Presentation and result windows

Every presentation entry point uses:

```text
ResultId -> ResultStore query or subscription
```

This includes View Data windows, output Pin View, input Pin View through its
upstream connection, reports, tables, plots, generic viewers, and execution
inspectors.

Window behavior by state:

- `Pending`: open immediately and show progress;
- `Ready`: choose a renderer from `ResultPresentation` and read the value;
- `Failed`: show the stored execution error and cause chain;
- `Cancelled`: show cancellation state;
- missing: show an internal consistency error with the `ResultId`.

An output Pin viewer defaults to the latest history occurrence regardless of
state and allows selecting any earlier occurrence. It does not silently select
the latest successful value. An input Pin viewer resolves its connection to the
upstream output history; input pins do not receive a duplicate history.

Adapters preserve presentation explicitly by default. An adapter that changes
semantic meaning must explicitly produce a new presentation. No presentation
is recovered later by graph traversal or payload guessing.

Report renderers validate only `Ready` values in their canonical form. A
validation failure records the result ID, presentation, actual value category,
producer node and output Pin, and exact failing field path. The UI may show a
short message; actionable diagnostics belong in `NOTIFY`, `APP`, or `EXEC`
logs.

## IPC boundary

IPC DTOs use `ResultId` as the data entry point and expose serializable state,
provenance, presentation, and appropriately sized value projections. They do
not expose:

- `ArtifactId` or artifact snapshots;
- internal tagged protocol enum serialization;
- View Data-specific snapshot payloads;
- presentation inferred through lineage;
- physical spill identifiers.

Small values may be returned directly. Large results use pagination, streaming,
or specialized result queries keyed by the same `ResultId` rather than forcing
full JSON serialization.

## Session lifecycle and retention

"Retain forever" means retain for the currently loaded project session:

- no automatic count-, time-, or memory-based eviction;
- Frame destruction, graph tab close, or result window close does not delete
  results;
- project close or project switch releases the complete `ResultStore`, Pin
  histories, memo entries, and physical storage;
- no history or result data is written into project files or a cross-session
  cache by this design.

An explicit user clear operation may be designed later. Until then, session
teardown is the only automatic reclamation boundary. The UI should make the
session-only lifetime clear if result-history controls are added.

## Migration strategy

The migration is ResultStore-first and proceeds in independently verifiable
stages. The existing artifact implementation may temporarily serve as a private
physical-storage adapter, but it must not remain a business identity and must
shrink at each stage.

### Stage 1: Establish the authoritative model

Introduce stable `ResultId`, `StoredResult`, the result state machine,
activation result groups, Pin history, group transitions, queries,
subscriptions, and session teardown. Temporarily bridge existing runtime values
into the new results without exposing artifact identities to new consumers.

### Stage 2: Convert Frame bindings

Replace `ValueRef -> RuntimeValue` with `ValueRef -> ResultId`. Preserve
control-flow and activation-local Frame responsibilities. Resolve all data
through the store and current activation bindings.

### Stage 3: Move scheduler and kernel boundaries

Create pending groups before execution, resolve all inputs, invoke the kernel
once, prepare all outputs outside the store lock, and commit the complete group
atomically. Preserve existing stream, channel, backpressure, cancellation,
spill, and materialization-budget mechanics behind pending writers.

### Stage 4: Convert memoization

Cache complete output `ResultId` vectors instead of runtime values. Add produced
and reused Pin usage records and enforce project-session scope.

### Stage 5: Convert windows, Pin View, services, and IPC

Make every presentation path query and subscribe by `ResultId`. Add scalable
large-value access methods. Remove artifact IDs and internal protocol wire
formats from DTOs.

### Stage 6: Simplify View Data

Have View Data open its input result directly. Remove its snapshot pin,
rematerialization, replay construction, result leaf, special publication, and
presentation-lineage behavior.

### Stage 7: Internalize physical storage

Move shared memory, spill resources, multiple readers, and pending builders
under `StoredValue`. Remove business-level artifact identities and ownership.

### Stage 8: Delete obsolete paths

After all consumers use `ResultId`, delete the old storage and publication
layers rather than preserving compatibility shims.

## Deletion and refactoring inventory

### Delete after replacement

From `src-tauri/src/node_system/runtime/artifact.rs`:

- `ArtifactSnapshot`;
- `ArtifactStore`;
- `ArtifactId`;
- `ArtifactDescriptor`;
- artifact publication guards;
- result-source holds;
- run-owned artifact lifecycle.

From `src-tauri/src/node_system/runtime/result_store.rs`:

- ownership of a separate `ArtifactStore`;
- `prepare_runtime_value`;
- `prepare_runtime_value_with_presentation`;
- `publish_runtime_value`;
- runtime-value-to-snapshot conversion;
- ResultSource ownership of data.

From `src-tauri/src/node_system/runtime/scheduler.rs`:

- complete `RuntimeValue` storage in Frames;
- `stage_result_sources` and equivalent old publication paths;
- View Data and adapter-specific reverse presentation lineage;
- per-output publication semantics for a normal activation.

From `src-tauri/src/node_system/runtime/kernels/core_nodes/debug.rs` and View
Data catalog support:

- snapshot data output;
- rematerialization and cursor replay;
- replayable artifact construction;
- View Data `FragmentResult` and `result_leaf` declarations.

From the persistent runtime-value model in
`src-tauri/src/node_system/runtime/run.rs`:

- `RuntimeValue::Artifact` as Frame or result identity;
- `Artifact` and `ArtifactKind` business concepts;
- `MaterializedArtifact` identity responsibilities.

If a short-lived kernel-call value type remains useful, it must not enter
Frames, histories, IPC, or persistent result state.

### Refactor rather than delete

- `ResultStore` becomes authoritative.
- `Frame` retains execution-local binding and control-flow responsibilities but
  stores `ResultId` references.
- `RunMemoization` stores complete output result-ID vectors.
- stream channels, cancellation, backpressure, spill, multiple readers, and
  materialization budgets retain their execution semantics.
- `ResultSourceId`, if still needed, may identify a presentation session only;
  it cannot own or identify the underlying data.

## Error diagnostics

User-facing messages can remain concise, but logs and diagnostics must identify
where a bad value entered the presentation boundary. A report-rendering error
must include at least:

- `ResultId`;
- `RunId` and `ActivationId`;
- producer `NodeId` and output `PinId` when public;
- `ResultPresentation`;
- actual stored value category;
- exact missing or invalid field path and reason.

A Frame binding that references a missing result is an internal consistency
failure. It must not be masked by looking up a Pin's previous successful value.

## Verification strategy

### ResultStore unit coverage

Verify:

- complete pending group creation and rollback on setup failure;
- one history entry per public output occurrence;
- atomic transition of all group outputs to `Ready`;
- inability to publish only part of a normal group;
- group transition to `Failed` or `Cancelled`;
- cancellation versus completion race has exactly one winner;
- terminal states cannot transition again;
- session history is not automatically evicted;
- project-session teardown releases results and indexes.

### Frame and scheduler coverage

Verify:

- Frame data bindings contain only `ResultId`;
- current activation bindings, not latest Pin history, resolve inputs;
- all inputs are supplied to one kernel invocation;
- all outputs are returned before publication;
- input and output mapping strictly follows `port_sequence`;
- downstream execution waits for the complete upstream group;
- failed and cancelled dependencies propagate without kernel invocation;
- no global lock is held during kernel work, I/O, or materialization.

### Storage and memoization coverage

Verify:

- downstream execution and viewers reference the same result;
- memoization reuses the same result IDs and records reuse;
- shared in-memory data is not copied;
- spill-backed values support multiple independent readers;
- removing Replayable as a business concept does not remove repeated-read
  capability;
- session teardown cleans private temporary storage.

### View Data regression coverage

For `OLS Summary.report -> View Data.data`, verify:

- the OLS output Pin, View Data input, automatic window, and Pin View use the
  exact same `ResultId`;
- View Data creates no snapshot result or replayable artifact;
- View Data performs no rematerialization;
- default execution opens the result window;
- presentation remains `Report(OlsSummary)`;
- the renderer receives the canonical report value;
- rendering failures include field-level diagnostics.

Continue exercising the external diagnostic project:

```text
C:\Users\zhou.yi31\Documents\New Project
events/New Event.yssbi-event
```

The regression must cover Default execution, not only PinPreview.

### Required checks during implementation

For each Rust stage, run focused tests plus:

```text
pnpm rust:fmt:check
pnpm rust:check
```

For frontend stages, run focused tests plus:

```text
pnpm typecheck
```

For stages spanning Rust and frontend, run:

```text
pnpm verify
git diff --check
```

## Final architecture

```text
Frame[ValueRef]
    -> ResultId
    -> ResultStore[ResultId]
    -> StoredResult
    -> StoredValue
```

Lightweight secondary indexes are:

```text
OutputPinId -> PinResultHistory -> ResultId
MemoKey -> [ResultId]
```

Neither index owns or duplicates the value. View Data, Pin View, reports,
plots, inspectors, downstream nodes, and memoization all converge on the same
result identity and the same physical value.