# Unified Result Store Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the split Frame/RuntimeValue/Artifact/ResultSource execution-result model with one project-session `ResultStore` whose `ResultId`s are shared by output Pin history, downstream execution, memoization, View Data, Pin View, and presentation windows.

**Architecture:** Create pending result groups before each kernel activation, bind `Frame` values to `ResultId`, invoke the kernel once with all inputs, prepare all outputs outside the store lock, and atomically complete the whole output group. Keep in-memory sharing, spill, independent readers, streaming, cancellation, and backpressure as private `StoredValue` capabilities; migrate all IPC and frontend presentation entry points to `ResultId`, then delete snapshots, replayable artifacts, source-release ownership, and View Data rematerialization.

**Tech Stack:** Rust 2024, Tauri 2, serde, React 19, TypeScript 5.8, Zustand, Vitest, existing YssBI runtime stream/spill/materialization infrastructure.

## Global Constraints

- `ResultStore` is the sole business owner of execution result state and values.
- One kernel activation consumes all required inputs, runs once, computes all declared outputs, and publishes the complete output group atomically.
- Every output data Pin receives an independent `ResultId`; all output mapping follows protocol `port_sequence` exactly.
- `Frame` stores `ValueRef -> ResultId`; it never owns complete values, artifacts, snapshots, or Pin history.
- Input Pins do not cache values or histories; runtime input resolution uses the current Frame binding, never a Pin's latest historical result.
- Result states are `Pending -> Ready | Failed | Cancelled`; terminal states are immutable and normal activations do not publish partial success.
- Pin history retains pending, ready, failed, cancelled, produced, and reused occurrences for the complete current project session without automatic eviction.
- Project close or switch releases all results, histories, memo entries, and private physical storage; no result history persists across sessions.
- Large values are physically stored once; shared memory and spill-backed multiple-reader support remain private `StoredValue` capabilities.
- Ordinary downstream kernels and renderers cannot consume partial pending data.
- View Data has `Enter`, `Data`, and `Then` only; it opens its exact input `ResultId` and does not produce, copy, replay, or rematerialize data.
- Rust owns result state and history; frontend stores are projections and UI state.
- Tauri commands remain thin; frontend invokes remain in `src/services/`.
- Do not retain legacy compatibility shims in this 0.x project.
- Run project commands from the repository root through `pnpm`; do not invoke Cargo directly.
- Do not run full `pnpm rust:test` by default; use focused Rust tests, `pnpm rust:check`, and `pnpm verify` for cross-stack delivery.
- Preserve unrelated user changes. Do not create commits unless the user explicitly authorizes commits during execution.

---

## File Structure Map

### Authoritative result domain

- Create `src-tauri/src/node_system/plan/result_presentation.rs`: runtime-neutral result presentation enums shared by compiler plans and runtime results.
- Create `src-tauri/src/node_system/runtime/result.rs`: `ResultId`, state, provenance, failure, progress, group, Pin-history, and usage domain types.
- Create `src-tauri/src/node_system/runtime/stored_value.rs`: direct value ownership, readers, metadata, paging, and the temporary private physical-storage bridge removed later in this plan.
- Rewrite `src-tauri/src/node_system/runtime/result_store.rs`: session authority, pending-group creation, atomic transitions, waiting/subscription, history, paging, and reuse.
- Modify `src-tauri/src/node_system/runtime/mod.rs`: export the authoritative result APIs and eventually remove artifact/source exports.
- Modify `src-tauri/src/project/project_store.rs`: keep `ResultStore` and session memoization at the project-session lifetime boundary.

### Plan, compiler, Frame, and scheduler

- Modify `src-tauri/src/node_system/plan/model.rs`: add public output identity and explicit presentation to `PlannedOutput`.
- Modify `src-tauri/src/node_system/compiler/pipeline.rs` and adapter lowering/finalization files: preserve `port_sequence`, output Pin identity, and presentation without reverse lineage inference.
- Modify `src-tauri/src/node_system/plan/validation.rs`: validate output identity/presentation and ordered activation descriptors.
- Modify `src-tauri/src/node_system/runtime/scheduler.rs`: bind Frames to IDs, create pending groups, resolve exact inputs, atomically complete groups, propagate failure/cancellation, and remove late source staging.
- Modify `src-tauri/src/node_system/runtime/scheduling.rs`: carry pending-group identity and prepared outputs in operation completion.
- Modify `src-tauri/src/node_system/runtime/run.rs`: make `RunResult` reference results and restrict full values to the kernel-call boundary.
- Modify `src-tauri/src/node_system/runtime/kernel.rs`: keep kernels as complete input-to-output transformations without result ownership.

### Physical storage, streaming, and memoization

- Modify `src-tauri/src/node_system/runtime/materialization.rs`: produce `StoredValue`, use pending writers/builders, and keep I/O outside result authority locks.
- Modify `src-tauri/src/node_system/runtime/spill.rs`: make spill storage private result backing with independent readers; remove `ReplayArtifact`.
- Modify `src-tauri/src/node_system/runtime/stream.rs`: preserve bounded channels, backpressure, cancellation, and deadline behavior.
- Modify `src-tauri/src/node_system/runtime/memoization.rs`: cache ordered `ResultId` vectors and preserve single-flight behavior.
- Delete `src-tauri/src/node_system/runtime/artifact.rs` only after all consumers use `StoredValue` and `ResultId`.

### Runtime events and Tauri boundary

- Modify `src-tauri/src/node_system/runtime/execution_event.rs`: add group/result/window events carrying `ResultId`.
- Modify `src-tauri/src/commands/node_system_execution_dto.rs`: serialize opaque IDs as decimal strings and expose result state/provenance/history without artifact identity.
- Modify `src-tauri/src/commands/command_node_system.rs`: replace source commands with ResultId queries and Pin-history queries.
- Modify `src-tauri/src/project/project_state.rs`: expose authoritative ResultId query methods and remove source-release lifecycle.
- Modify `src-tauri/src/lib.rs`: register only the new result commands.

### Frontend result contract and views

- Create `src/shared/types/dto/result.ts`: strict result descriptor, state, provenance, value, page, presentation, and Pin-history types.
- Create `src/shared/types/dto/resultParser.ts`: strict parsers for backend result DTOs.
- Create `src/services/result/resultService.ts`: all ResultId and Pin-history invokes.
- Modify `src/shared/types/dto/runEvent.ts` and `src/shared/types/dto/runEventParser.ts`: consume `resultId`, group changes, and View Data open requests.
- Modify `src/features/application/editor/observeGraphRunEvent.ts`, `requestPinPreview.ts`, and `useProjectOperations.ts`: use exact result IDs and open View Data from a backend request.
- Modify `src/features/core/execution/pinViewTarget.ts`, `pinResultIndex.ts`, `normalizePinResult.ts`, `useExecutionStore.ts`, and `src/shared/types/ui/execution.ts`: store result/history projections only.
- Modify `src/features/core/resultSource/inspectableSource.ts` and presentation-window application files: route and load by `resultId`, show every result state, and remove release-on-close.
- Rename/remove old `resultSource` DTO/service files only after every caller has migrated.

### View Data and final removal

- Modify `src-tauri/src/node_system/catalog/core_nodes/debug.rs`: remove `snapshot` and `result_leaf` from View Data.
- Modify `src-tauri/src/node_system/runtime/kernels/core_nodes/debug.rs`: remove `ViewKernel` rematerialization; View Data becomes a scheduler side effect using its input ID.
- Modify `src-tauri/src/project/production_tests.rs`: make the real OLS → View Data regression assert one shared `ResultId`.
- Remove old ResultSource, ArtifactSnapshot, ArtifactStore, Replayable, release, reverse-lineage, and report sequence-unwrapping paths after the regression is green.

---

### Task 1: Explicit Ordered Output Result Metadata

**Files:**
- Create: `src-tauri/src/node_system/plan/result_presentation.rs`
- Modify: `src-tauri/src/node_system/plan/mod.rs`
- Modify: `src-tauri/src/node_system/plan/model.rs:280-309`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs`
- Modify: `src-tauri/src/node_system/compiler/specialization/finalization.rs`
- Modify: `src-tauri/src/node_system/plan/validation.rs`
- Test: `src-tauri/src/node_system/compiler/pipeline.rs`
- Test: `src-tauri/src/node_system/plan/tests.rs`

**Interfaces:**
- Consumes: `GraphOutputRef`, `PortAddress`, `ResolvedNode.port_sequence`, `PlannedOutput`, and existing report mappings currently in `scheduler::report_kind_for_node_type`.
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResultPresentation {
    #[default]
    Inspector,
    Plot { chart: ResultPlotKind },
    Report { report: ResultReportKind },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedOutput {
    pub value: ValueRef,
    pub contract: PlannedValueContract,
    pub production: OutputProduction,
    pub public_output: Option<GraphOutputRef>,
    pub presentation: ResultPresentation,
}
```

- Public outputs carry their exact opaque graph path and `PortAddress`; compiler-inserted adapter outputs use `None`.
- Adapter lowering copies presentation explicitly unless the adapter changes semantic meaning.

- [ ] **Step 1: Write failing compiler and plan tests**

Add tests proving non-alphabetical protocol order, public Pin identity, and adapter presentation propagation:

```rust
#[test]
fn planned_outputs_preserve_protocol_order_identity_and_presentation() {
    let plan = compile_non_alphabetical_report_fixture();
    let operation = plan.operations.iter()
        .find(|operation| operation.source_node_type_id.as_str() == "test.report")
        .unwrap();

    assert_eq!(
        operation.outputs.iter().map(|output| output.public_output.as_ref().unwrap().port.port_key.as_str()).collect::<Vec<_>>(),
        ["z_result", "a_report"],
    );
    assert_eq!(operation.outputs[0].presentation, ResultPresentation::Inspector);
    assert_eq!(
        operation.outputs[1].presentation,
        ResultPresentation::Report { report: ResultReportKind::OlsSummary },
    );
}

#[test]
fn adapter_output_has_no_public_pin_and_inherits_presentation() {
    let plan = compile_report_through_adapter_fixture();
    let adapter = plan.operations.iter()
        .find(|operation| matches!(operation.kernel, PlannedKernel::Adapter(_)))
        .unwrap();
    assert!(adapter.outputs[0].public_output.is_none());
    assert_eq!(
        adapter.outputs[0].presentation,
        ResultPresentation::Report { report: ResultReportKind::OlsSummary },
    );
}
```

Use existing compiler fixture builders; do not construct an `ExecutionPlan` that bypasses lowering.

- [ ] **Step 2: Run the focused tests to verify RED**

Run:

```sh
pnpm rust:test --lib planned_outputs_preserve_protocol_order_identity_and_presentation
pnpm rust:test --lib adapter_output_has_no_public_pin_and_inherits_presentation
```

Expected: compile failure because `PlannedOutput.public_output`, `PlannedOutput.presentation`, and `ResultPresentation` do not exist.

- [ ] **Step 3: Add result presentation and lower ordered output metadata**

Define `ResultPresentation`, `ResultReportKind`, and `ResultPlotKind` in `src-tauri/src/node_system/plan/result_presentation.rs` and re-export them from `plan/mod.rs`. Extend `PlannedOutput`, and construct outputs by iterating resolved `port_sequence`, never map keys:

```rust
let outputs = resolved.port_sequence.iter().filter_map(|port_key| {
    let port = resolved.ports.get(port_key)?;
    if port.direction != PortDirection::Output || port.kind != PortKind::Data {
        return None;
    }
    Some(PlannedOutput {
        value: value_for_port(&port.address)?,
        contract: contract_for_port(port)?,
        production: port.output_production?,
        public_output: Some(GraphOutputRef {
            graph_path: graph_path.clone(),
            port: port.address.clone(),
        }),
        presentation: presentation_for_output(node_type_id, port_key),
    })
}).collect::<Result<Vec<_>, CompileError>>()?.into_boxed_slice();
```

Move the scheduler's report mapping into `presentation_for_output`. Make the mapping output-specific so a multi-output node can expose Inspector and Report outputs independently.

- [ ] **Step 4: Add plan validation**

Reject duplicated public output identities and presentation assigned to a non-data output. Validate that output arrays retain the lowered sequence:

```rust
let mut public_outputs = BTreeSet::new();
for output in &operation.outputs {
    if let Some(address) = &output.public_output
        && !public_outputs.insert(address.clone())
    {
        return Err(PlanValidationError::DuplicatePublicOutput(address.clone()));
    }
}
```

- [ ] **Step 5: Run focused and compiler validation**

Run:

```sh
pnpm rust:test --lib semantic_graph_preserves_protocol_port_order_for_kernel_abi
pnpm rust:test --lib planned_outputs_preserve_protocol_order_identity_and_presentation
pnpm rust:test --lib adapter_output_has_no_public_pin_and_inherits_presentation
pnpm rust:check
```

Expected: all commands exit 0.

- [ ] **Step 6: Commit only if execution-time authorization explicitly permits it**

```sh
git add src-tauri/src/node_system/plan src-tauri/src/node_system/compiler
GIT_EDITOR=true git commit -m "Add ordered result output metadata"
```

Otherwise leave the verified task changes uncommitted and continue.

---

### Task 2: Authoritative Result Domain and Atomic Store

**Files:**
- Create: `src-tauri/src/node_system/runtime/result.rs`
- Create: `src-tauri/src/node_system/runtime/stored_value.rs`
- Rewrite: `src-tauri/src/node_system/runtime/result_store.rs`
- Modify: `src-tauri/src/node_system/runtime/mod.rs`
- Modify: `src-tauri/src/project/project_store.rs`
- Test: `src-tauri/src/node_system/runtime/result_store.rs`

**Interfaces:**
- Consumes: Task 1 `ResultPresentation`, `GraphOutputRef`, `RunId`, `ActivationId`, `GraphRevision`, current shared `Artifact` storage as a strictly private transitional backing.
- Produces:

```rust
pub struct ResultId(u64);

pub struct ResultProvenance {
    pub run_id: RunId,
    pub activation_id: ActivationId,
    pub graph_path: GraphResourcePath,
    pub graph_revision: GraphRevision,
    pub node_id: NodeId,
    pub output: Option<GraphOutputRef>,
    pub created_at_ms: u64,
}

pub enum ResultState {
    Pending(ResultProgress),
    Ready(StoredValue),
    Failed(Arc<ResultFailure>),
    Cancelled,
}

pub struct PendingOutputDescriptor {
    pub value: ValueRef,
    pub output: Option<GraphOutputRef>,
    pub presentation: ResultPresentation,
    pub contract: PlannedValueContract,
}

pub struct ActivationResultGroup {
    pub activation_id: ActivationId,
    pub output_result_ids: Box<[ResultId]>,
}

pub struct PinResultEntry {
    pub result_id: ResultId,
    pub run_id: RunId,
    pub activation_id: ActivationId,
    pub graph_revision: GraphRevision,
    pub created_at_ms: u64,
    pub usage: ResultUsage,
}

impl ResultStore {
    pub fn create_pending_group(
        &self,
        provenance: ActivationProvenance,
        outputs: &[PendingOutputDescriptor],
    ) -> Result<ActivationResultGroup, ResultStoreError>;
    pub fn complete_group(
        &self,
        group: &ActivationResultGroup,
        values: Box<[StoredValue]>,
    ) -> Result<(), ResultStoreError>;
    pub fn fail_group(
        &self,
        group: &ActivationResultGroup,
        failure: Arc<ResultFailure>,
    ) -> Result<(), ResultStoreError>;
    pub fn cancel_group(&self, group: &ActivationResultGroup)
        -> Result<(), ResultStoreError>;
    pub fn result(&self, id: ResultId) -> Option<StoredResult>;
    pub fn pin_history(&self, output: &GraphOutputRef) -> Box<[PinResultEntry]>;
}
```

- `StoredValue` may temporarily contain the current shared `Artifact` object internally, but no `ArtifactId`, snapshot, source hold, or physical path appears in result APIs.
- Store mutation occurs under one short authority lock; notifications happen after unlock.

- [ ] **Step 1: Replace old source-store tests with failing result-group tests**

Add the following exact behaviors:

```rust
#[test]
fn pending_group_allocates_ordered_results_and_pin_history() {
    let store = ResultStore::new();
    let outputs = test_outputs(["z_result", "a_report"]);
    let group = store.create_pending_group(test_provenance(7), &outputs).unwrap();

    assert_eq!(group.output_result_ids.len(), 2);
    assert_ne!(group.output_result_ids[0], group.output_result_ids[1]);
    assert!(matches!(store.result(group.output_result_ids[0]).unwrap().state, ResultState::Pending(_)));
    assert_eq!(store.pin_history(outputs[0].output.as_ref().unwrap())[0].result_id, group.output_result_ids[0]);
}

#[test]
fn complete_group_is_all_or_nothing_and_terminal() {
    let store = ResultStore::new();
    let group = store.create_pending_group(test_provenance(9), &test_outputs(["left", "right"])).unwrap();

    assert!(store.complete_group(&group, vec![StoredValue::scalar(Value::Null)].into_boxed_slice()).is_err());
    assert!(group.output_result_ids.iter().all(|id| matches!(store.result(*id).unwrap().state, ResultState::Pending(_))));

    store.complete_group(&group, vec![StoredValue::scalar(Value::Null), StoredValue::scalar(Value::Null)].into_boxed_slice()).unwrap();
    assert!(group.output_result_ids.iter().all(|id| matches!(store.result(*id).unwrap().state, ResultState::Ready(_))));
    assert!(store.cancel_group(&group).is_err());
}

#[test]
fn result_store_never_evicts_within_the_session() {
    let store = ResultStore::new();
    let first = create_ready_test_group(&store, 1);
    for activation in 2..=5000 { create_ready_test_group(&store, activation); }
    assert!(store.result(first.output_result_ids[0]).is_some());
}
```

Also add setup rollback, fail/cancel whole group, completion-vs-cancel race, history of terminal occurrences, and store-drop spill release tests. Delete `result_sources_evict_oldest_entry_at_capacity`; it asserts forbidden behavior.

- [ ] **Step 2: Run the focused tests to verify RED**

Run:

```sh
pnpm rust:test --lib pending_group_allocates_ordered_results_and_pin_history
pnpm rust:test --lib complete_group_is_all_or_nothing_and_terminal
pnpm rust:test --lib result_store_never_evicts_within_the_session
```

Expected: compile failure because the authoritative result types and methods do not exist.

- [ ] **Step 3: Implement the result domain and direct store registry**

Use one registry lock and process-global monotonically allocated opaque IDs:

```rust
struct ResultStoreRegistry {
    results: BTreeMap<ResultId, StoredResult>,
    groups: BTreeMap<ActivationId, ActivationResultGroup>,
    pin_history: BTreeMap<GraphOutputRef, Vec<PinResultEntry>>,
}

#[derive(Clone)]
pub struct ResultStore {
    inner: Arc<ResultStoreInner>,
}

struct ResultStoreInner {
    registry: Mutex<ResultStoreRegistry>,
    changed: Condvar,
}
```

`create_pending_group` validates all descriptors and ID allocation before mutating any map. It then inserts the complete group and all public Pin entries in one lock scope. Do not retain `max_sources`, capacity eviction, run-owned holds, or window release.

- [ ] **Step 4: Implement atomic terminal transitions and readers**

Use one shared helper so complete/fail/cancel cannot diverge:

```rust
fn transition_group(
    &self,
    group: &ActivationResultGroup,
    transition: impl FnMut(usize, &StoredResult) -> Result<ResultState, ResultStoreError>,
) -> Result<(), ResultStoreError>;
```

The helper first validates every ID, activation, state, and output count without mutation; then updates all records; then releases the lock, notifies waiters, and emits any callback. `result()` clones only `Arc`-backed metadata/value handles.

- [ ] **Step 5: Preserve current paging behind `StoredValue` without exposing artifacts**

Provide:

```rust
impl StoredValue {
    pub fn scalar(value: Value) -> Self;
    pub fn kind(&self) -> StoredValueKind;
    pub fn len(&self) -> usize;
    pub fn page(&self, offset: usize, limit: usize) -> Result<Box<[Value]>, StoredValueReadError>;
    pub fn open_reader(&self) -> Result<StoredValueReader, StoredValueReadError>;
}
```

Initially delegate shared artifact reads privately. Do not create `ArtifactSnapshot`, `ArtifactId`, or ResultSource holds in these methods.

- [ ] **Step 6: Run ResultStore validation**

Run:

```sh
pnpm rust:test --lib node_system::runtime::result_store::tests
pnpm rust:fmt:check
pnpm rust:check
git diff --check
```

Expected: tests report 0 failures and all checks exit 0.

- [ ] **Step 7: Commit only if explicitly authorized**

```sh
git add src-tauri/src/node_system/runtime src-tauri/src/project/project_store.rs
GIT_EDITOR=true git commit -m "Add authoritative session result store"
```

---

### Task 3: ResultId Frames and Activation-Atomic Scheduling

**Files:**
- Modify: `src-tauri/src/node_system/runtime/scheduler.rs:155-204,626-657,995-1200,1890-2600,3111-3210`
- Modify: `src-tauri/src/node_system/runtime/scheduling.rs`
- Modify: `src-tauri/src/node_system/runtime/run.rs`
- Modify: `src-tauri/src/node_system/runtime/kernel.rs`
- Test: `src-tauri/src/node_system/runtime/tests.rs`
- Test: `src-tauri/src/node_system/runtime/scheduler.rs`

**Interfaces:**
- Consumes: Task 2 `ResultStore`, `ResultId`, `ActivationResultGroup`, `StoredValue`, Task 1 ordered `PlannedOutput` metadata.
- Produces:

```rust
struct Frame {
    id: FrameId,
    bindings: Vec<Option<ResultId>>,
    attempted: BTreeMap<MemoKey, AttemptId>,
    completed: BTreeSet<MemoKey>,
    completion_counts: BTreeMap<OperationIndex, usize>,
}

struct PreparedOperation {
    operation: OperationIndex,
    activation: ActivationId,
    input_result_ids: Box<[ResultId]>,
    output_group: ActivationResultGroup,
    // existing attempt, memo, and workload fields
}

pub struct OperationCompletion {
    pub operation: OperationIndex,
    pub activation: ActivationId,
    pub output_group: ActivationResultGroup,
    pub outputs: Result<Box<[StoredValue]>, RunError>,
}
```

- Operation-local transient kernel inputs may contain readers/streams, but they cannot enter Frame, ResultStore history, or memoization.

- [ ] **Step 1: Add failing Frame and scheduler behavior tests**

Add tests with checkpoint kernels:

```rust
#[test]
fn frame_bindings_are_result_ids_and_do_not_own_values() {
    let store = ResultStore::new();
    let id = create_ready_scalar(&store, Value::Integer(42));
    let mut frame = Frame::new(1);
    frame.bind_result(ValueRef::new(0), id).unwrap();
    assert_eq!(frame.result_id(ValueRef::new(0)).unwrap(), id);
    frame.clear_region_values([ValueRef::new(0)]);
    assert!(store.result(id).is_some());
}

#[test]
fn kernel_receives_all_inputs_once_and_outputs_publish_atomically() {
    let fixture = MultiInputMultiOutputFixture::run();
    assert_eq!(fixture.kernel_call_count(), 1);
    assert_eq!(fixture.observed_inputs(), [Value::Integer(2), Value::Integer(3)]);
    assert_eq!(fixture.output_states_at_commit_checkpoint(), [ResultStateTag::Pending, ResultStateTag::Pending]);
    assert_eq!(fixture.final_output_values(), [Value::Integer(5), Value::Integer(6)]);
}

#[test]
fn scheduler_uses_current_frame_binding_not_latest_pin_history() {
    let fixture = CompetingActivationFixture::run();
    assert_eq!(fixture.consumed_result_id(), fixture.current_frame_result_id());
    assert_ne!(fixture.consumed_result_id(), fixture.latest_history_result_id());
}
```

Also add tests that failed input fails all outputs without invoking the kernel, cancelled input cancels all outputs, a downstream operation waits for the complete upstream group, and output count mismatch leaves the group failed rather than partially ready.

- [ ] **Step 2: Run focused tests to verify RED**

Run:

```sh
pnpm rust:test --lib frame_bindings_are_result_ids_and_do_not_own_values
pnpm rust:test --lib kernel_receives_all_inputs_once_and_outputs_publish_atomically
pnpm rust:test --lib scheduler_uses_current_frame_binding_not_latest_pin_history
```

Expected: compile failure because Frame and scheduling still use `RuntimeValue`.

- [ ] **Step 3: Convert Frame and structured-control transfers to IDs**

Replace `values` and its methods with ID bindings:

```rust
fn bind_result(&mut self, value: ValueRef, result_id: ResultId) -> Result<(), RunError>;
fn result_id(&self, value: ValueRef) -> Result<ResultId, RunError>;
fn copy_result(&mut self, source: ValueRef, destination: ValueRef) -> Result<(), RunError>;
```

Insert bound literals/defaults as internal ready results before binding. Function call arguments/results, branch outputs, loop carried values, and adapters copy `ResultId`; they never clone complete stored values. Remove `Frame::close_streams`; streams become operation-local.

- [ ] **Step 4: Create pending groups before kernel execution**

In `prepare_operation`, resolve exact input IDs and create all output descriptors from `operation.outputs` in array order:

```rust
let descriptors = operation.outputs.iter().map(|output| PendingOutputDescriptor {
    value: output.value,
    output: output.public_output.clone(),
    presentation: output.presentation,
    contract: output.contract.clone(),
}).collect::<Vec<_>>();
let output_group = results.create_pending_group(
    activation_provenance(run_id, activation, operation, plan),
    &descriptors,
)?;
for (output, result_id) in operation.outputs.iter().zip(output_group.output_result_ids.iter().copied()) {
    frame.bind_result(output.value, result_id)?;
}
```

If a required exact input is failed or cancelled, create the current output group and transition it without invoking the kernel. Pending exact inputs block dependency readiness rather than falling back to Pin history.

- [ ] **Step 5: Invoke once, prepare all outputs, and atomically commit**

Keep kernel execution outside the store lock. Validate complete output count and contracts before calling:

```rust
results.complete_group(&completion.output_group, completion.outputs?)?;
```

On ordinary error call `fail_group`; on cancellation call `cancel_group`. Wake dependent operations only after the group transition completes. Preserve admission, retry, deadline, trace, and backpressure behavior.

- [ ] **Step 6: Remove late duplicate result publication**

Delete:

```text
PendingSourceEvent
PendingSourcePublication
RunExecutor::stage_result_sources
RunExecutor::commit_result_sources
SchedulerCheckpoint::ResultSourceStaged
report_kind_for_value
result_presentation
```

Change named `RunResult.values` to `BTreeMap<Box<str>, ResultId>` if named graph results remain required. `finish_run` must no longer convert Frame values into snapshots or ResultSources.

- [ ] **Step 7: Prove locks are short and execution remains atomic**

Add a checkpoint test that blocks materialization while another thread calls `ResultStore::result`; the query must return promptly with `Pending`. Then run:

```sh
pnpm rust:test --lib frame_bindings_are_result_ids_and_do_not_own_values
pnpm rust:test --lib kernel_receives_all_inputs_once_and_outputs_publish_atomically
pnpm rust:test --lib scheduler_uses_current_frame_binding_not_latest_pin_history
pnpm rust:test --lib bounded_materialization_producer_panic_is_not_partial_success
pnpm rust:fmt:check
pnpm rust:check
```

Expected: all commands exit 0.

- [ ] **Step 8: Commit only if explicitly authorized**

```sh
git add src-tauri/src/node_system/runtime
GIT_EDITOR=true git commit -m "Bind scheduler frames to result IDs"
```

---

### Task 4: StoredValue Materialization, Spill, and Independent Readers

**Files:**
- Modify: `src-tauri/src/node_system/runtime/stored_value.rs`
- Modify: `src-tauri/src/node_system/runtime/materialization.rs`
- Modify: `src-tauri/src/node_system/runtime/spill.rs`
- Modify: `src-tauri/src/node_system/runtime/run.rs`
- Modify: `src-tauri/src/node_system/runtime/stream.rs`
- Test: `src-tauri/src/node_system/runtime/tests.rs`

**Interfaces:**
- Consumes: Task 2 `StoredValue`, Task 3 operation-local output preparation, existing `MemoryReservation`, `RunResourceOwner`, bounded streams, spill quota, `SpillCursor`.
- Produces:

```rust
pub enum StoredValue {
    Scalar(Value),
    InMemory(Arc<InMemoryStorage>),
    SpillBacked(Arc<SpillStorage>),
}

pub(crate) struct PendingValueWriter {
    result_id: ResultId,
    builder: StorageBuilder,
}

impl PendingValueWriter {
    pub fn push(&mut self, value: Value) -> Result<(), RunError>;
    pub fn finish(self) -> Result<StoredValue, RunError>;
}
```

- `InMemoryStorage` owns one shared immutable payload, metadata, and one `MemoryReservation`.
- `SpillStorage` owns the durable session-lifetime spill resource and creates a fresh reader per call.

- [ ] **Step 1: Rewrite physical-storage tests against StoredValue**

Replace replay/artifact assertions with direct result backing assertions:

```rust
#[test]
fn spill_backed_stored_value_supports_two_independent_passes() {
    let stored = spill_stored_values([Value::Integer(1), Value::Integer(2)]);
    let first = stored.open_reader().unwrap().collect::<Result<Vec<_>, _>>().unwrap();
    let second = stored.open_reader().unwrap().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(first, second);
    assert_eq!(first, [Value::Integer(1), Value::Integer(2)]);
}

#[test]
fn in_memory_clones_share_one_payload_and_reservation() {
    let (stored, budget) = reserved_stored_values(64);
    let clone = stored.clone();
    assert!(stored.ptr_eq(&clone));
    assert_eq!(budget.reserved_bytes(), 64);
    drop(stored);
    assert_eq!(budget.reserved_bytes(), 64);
    drop(clone);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn cancelled_pending_writer_removes_uncommitted_spill() {
    let path = write_pending_spill_then_cancel();
    assert!(!path.exists());
}
```

Preserve existing backpressure, threshold, stable order, typed fidelity, quota rollback, producer panic, deadline, and project-drain cleanup assertions.

- [ ] **Step 2: Run focused tests to verify RED**

Run:

```sh
pnpm rust:test --lib spill_backed_stored_value_supports_two_independent_passes
pnpm rust:test --lib in_memory_clones_share_one_payload_and_reservation
pnpm rust:test --lib cancelled_pending_writer_removes_uncommitted_spill
```

Expected: compile failure because direct StoredValue storage and pending writers are not complete.

- [ ] **Step 3: Move physical ownership under StoredValue**

Move in-memory metadata/reservation and spill backing out of `Artifact` business types. Keep spill internals private:

```rust
impl StoredValue {
    pub fn open_reader(&self) -> Result<StoredValueReader, StoredValueReadError> {
        match self {
            Self::Scalar(value) => Ok(StoredValueReader::one(value.clone())),
            Self::InMemory(storage) => Ok(storage.reader()),
            Self::SpillBacked(storage) => storage.reader(),
        }
    }
}
```

Rename `SpillArtifact` to private `SpillStorage`. Delete `ReplayArtifact`; repeated reads already come from fresh spill cursors.

- [ ] **Step 4: Refactor materialization to return complete StoredValue**

Change `materialize_values` and adapter materialization to return `Result<StoredValue, RunError>`. A `PendingValueWriter` accumulates privately and yields one complete value. It never updates ResultStore until all activation writers have finished.

Promotion order must be:

```text
finish writer -> promote private spill -> construct StoredValue
-> complete activation group -> clean remaining run resources
```

If cancellation or failure wins, dropping the uncommitted StoredValue removes its file. If completion wins, ResultStore's `Arc<SpillStorage>` owns it until project-session teardown.

- [ ] **Step 5: Remove Replay materialization semantics**

Delete `ArtifactKind::Replayable`, `MaterializedArtifact::Replayable`, and runtime `PlannedAdapter::Replay` construction. If an adapter still requests reusable storage, implement it as identity for already reusable `StoredValue` or ordinary materialization into `InMemory`/`SpillBacked`; do not introduce another result kind.

- [ ] **Step 6: Run preservation coverage**

Run:

```sh
pnpm rust:test --lib spill_backed_stored_value_supports_two_independent_passes
pnpm rust:test --lib spill_memory_threshold_preserves_stable_disk_order
pnpm rust:test --lib spilled_data_series_is_pageable_as_data_series
pnpm rust:test --lib result_store_paging_propagates_spill_read_failures
pnpm rust:test --lib bounded_materialization_capacity_one_applies_backpressure
pnpm rust:test --lib bounded_materialization_cleanup_covers_success_error_cancel_and_deadline
pnpm rust:test --lib cancelled_pending_writer_removes_uncommitted_spill
pnpm rust:fmt:check
pnpm rust:check
```

Expected: all commands exit 0.

- [ ] **Step 7: Commit only if explicitly authorized**

```sh
git add src-tauri/src/node_system/runtime
GIT_EDITOR=true git commit -m "Internalize result value storage"
```

---

### Task 5: Project-Session ResultId Memoization

**Files:**
- Modify: `src-tauri/src/node_system/runtime/memoization.rs`
- Modify: `src-tauri/src/node_system/runtime/scheduler.rs`
- Modify: `src-tauri/src/project/project_store.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Test: `src-tauri/src/node_system/runtime/memoization.rs`
- Test: `src-tauri/src/node_system/runtime/tests.rs`

**Interfaces:**
- Consumes: Task 2 `ResultId`, `ResultUsage`, `ResultStore::record_reused_group`, Task 4 direct StoredValue fingerprinting.
- Produces:

```rust
pub struct SessionMemoization {
    state: Mutex<SessionMemoizationState>,
}

enum FlightState {
    Running,
    Complete(Box<[ResultId]>),
    Failed,
}

impl ResultStore {
    pub fn record_reused_group(
        &self,
        provenance: ActivationProvenance,
        outputs: &[PendingOutputDescriptor],
        result_ids: &[ResultId],
    ) -> Result<(), ResultStoreError>;
}
```

- `ProjectStore` owns `Arc<SessionMemoization>` beside `ResultStore`; validation scratch owns an isolated memo/store pair.

- [ ] **Step 1: Write failing ID reuse tests**

```rust
#[test]
fn memoization_reuses_ordered_output_result_ids_and_history() {
    let fixture = MemoizedTwoOutputFixture::run_twice();
    assert_eq!(fixture.kernel_call_count(), 1);
    assert_eq!(fixture.first_result_ids(), fixture.second_result_ids());
    assert!(fixture.second_history().iter().all(|entry| matches!(entry.usage, ResultUsage::Reused { .. })));
}

#[test]
fn memoization_reuses_spill_backed_results_without_copying() {
    let fixture = MemoizedSpillFixture::run_twice();
    assert_eq!(fixture.first_result_id(), fixture.second_result_id());
    assert!(fixture.first_and_second_values_share_storage());
}

#[test]
fn replacing_project_session_invalidates_all_memo_entries() {
    let old_ids = run_memoized_project_then_replace();
    assert!(old_ids.iter().all(|id| current_project_result(*id).is_none()));
}
```

- [ ] **Step 2: Run focused tests to verify RED**

Run:

```sh
pnpm rust:test --lib memoization_reuses_ordered_output_result_ids_and_history
pnpm rust:test --lib memoization_reuses_spill_backed_results_without_copying
pnpm rust:test --lib replacing_project_session_invalidates_all_memo_entries
```

Expected: tests fail because completed flights still own `RuntimeValue[]` and memoization is run-scoped.

- [ ] **Step 3: Store ResultId vectors and fingerprint through ResultStore**

Replace `FlightState::Complete(Box<[RuntimeValue]>)` with `Box<[ResultId]>`. Change `OperationMemoKey::from_inputs` to accept exact input IDs plus `&ResultStore`, reading immutable `StoredValue` metadata/content without using physical spill paths or artifact kinds.

- [ ] **Step 4: Move memo authority to ProjectStore and record reuse**

Add:

```rust
pub results: ResultStore,
pub memoization: Arc<SessionMemoization>,
```

to `ProjectStore`. On cache hit, bind the existing IDs into the Frame and call `record_reused_group`; do not create new StoredResults, copy values, or replace original provenance. Session drain wakes running flights before dropping store and memoization.

- [ ] **Step 5: Run memoization and drain tests**

Run:

```sh
pnpm rust:test --lib memoization_reuses_ordered_output_result_ids_and_history
pnpm rust:test --lib memoization_reuses_spill_backed_results_without_copying
pnpm rust:test --lib per_run_memoization_drop_wakes_owned_flight
pnpm rust:test --lib bounded_materialization_cleanup_precedes_project_replacement_drain_completion
pnpm rust:fmt:check
pnpm rust:check
```

Expected: all commands exit 0. Rename the old per-run test when the type rename lands, preserving its wakeup assertion.

- [ ] **Step 6: Commit only if explicitly authorized**

```sh
git add src-tauri/src/node_system/runtime src-tauri/src/project
GIT_EDITOR=true git commit -m "Memoize session result IDs"
```

---

### Task 6: ResultId Runtime Events, Tauri DTOs, and Commands

**Files:**
- Modify: `src-tauri/src/node_system/runtime/execution_event.rs:1-48`
- Modify: `src-tauri/src/commands/node_system_execution_dto.rs:1-300`
- Modify: `src-tauri/src/commands/command_node_system.rs:769-940`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/commands/command_node_system.rs`
- Test: `src-tauri/src/commands/node_system_execution_dto.rs`

**Interfaces:**
- Consumes: authoritative ResultId descriptors/pages/history and activation-group notifications.
- Produces:

```rust
pub enum RunEventKind {
    // existing run and operation events
    ResultGroupChanged { activation_id: u64, result_ids: Box<[ResultId]> },
    OutputResultChanged {
        output: GraphOutputRef,
        generation: Option<u64>,
        result_id: ResultId,
    },
    OpenResultWindow { result_id: ResultId },
}

#[tauri::command]
pub fn get_result_descriptor(state: State<'_, ProjectState>, result_id: String)
    -> Result<Option<ResultDescriptorDto>, AppError>;
#[tauri::command]
pub fn get_result_value(state: State<'_, ProjectState>, result_id: String)
    -> Result<Option<ResultValueDto>, AppError>;
#[tauri::command]
pub fn get_result_page(state: State<'_, ProjectState>, result_id: String, offset: usize, limit: usize)
    -> Result<Option<ResultPageDto>, AppError>;
#[tauri::command]
pub fn get_pin_result_history(state: State<'_, ProjectState>, graph_path: String, pin_id: String)
    -> Result<Box<[PinResultEntryDto]>, AppError>;
```

- Every opaque u64 is serialized as a decimal string.
- Descriptor state includes pending progress, ready metadata, failed diagnostics, or cancelled state; no `artifactId` or physical path is serialized.

- [ ] **Step 1: Write failing DTO and command contract tests**

```rust
#[test]
fn result_dto_serializes_identity_state_and_provenance_without_artifacts() {
    let json = serde_json::to_value(ResultDescriptorDto::from(test_failed_result())).unwrap();
    assert_eq!(json["resultId"], "17");
    assert_eq!(json["state"]["kind"], "failed");
    assert_eq!(json["provenance"]["activationId"], "9");
    assert!(json.get("artifactId").is_none());
    assert!(json.to_string().find("spill").is_none());
}

#[test]
fn stale_result_id_cannot_alias_replacement_project() {
    let (state, old_result_id) = project_with_ready_result();
    replace_project(&state);
    assert!(get_result_descriptor_from_state(&state, &old_result_id.to_string()).unwrap().is_none());
}

#[test]
fn pin_history_command_returns_latest_failure_not_latest_success() {
    let state = project_with_success_then_failure();
    let history = get_pin_result_history_from_state(&state, "events/test.yssbi-event", TEST_PIN).unwrap();
    assert_eq!(history.last().unwrap().state_kind(), ResultStateKindDto::Failed);
}
```

- [ ] **Step 2: Run focused tests to verify RED**

Run:

```sh
pnpm rust:test --lib result_dto_serializes_identity_state_and_provenance_without_artifacts
pnpm rust:test --lib stale_result_id_cannot_alias_replacement_project
pnpm rust:test --lib pin_history_command_returns_latest_failure_not_latest_success
```

Expected: compile failure because ResultId DTOs and commands do not exist.

- [ ] **Step 3: Implement state/provenance/value/history DTOs**

Use tagged camelCase DTOs:

```rust
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResultStateDto {
    Pending { progress: ResultProgressDto },
    Ready,
    Failed { failure: ResultFailureDto },
    Cancelled,
}
```

`ResultDescriptorDto` contains `result_id`, state, provenance, presentation, value category, metadata, total count, and title. `ResultValueDto` uses plain JSON protocol values, retaining the existing internal-to-plain JSON conversion fix.

- [ ] **Step 4: Replace source commands and release lifecycle**

Replace `get_result_source_*` with `get_result_*` and add Pin history. Delete `release_result_source` and `release_run_result_sources`; closing a window or finishing a run cannot delete results. Update `ProjectState` methods and `tauri::generate_handler!` registration.

- [ ] **Step 5: Emit activation and presentation events with IDs**

Replace `ResultReady/OutputReady.source_id` with the new events. Keep Pin Preview `generation` only for correlation. Group events are emitted after atomic terminal transitions; View Data's `OpenResultWindow` carries only its exact input ID.

- [ ] **Step 6: Run Rust wire validation**

Run:

```sh
pnpm rust:test --lib result_dto_serializes_identity_state_and_provenance_without_artifacts
pnpm rust:test --lib stale_result_id_cannot_alias_replacement_project
pnpm rust:test --lib pin_history_command_returns_latest_failure_not_latest_success
pnpm rust:test --lib execution_ipc_dto_serializes_opaque_ids_as_decimal_strings
pnpm rust:fmt:check
pnpm rust:check
```

Expected: all commands exit 0.

- [ ] **Step 7: Commit only if explicitly authorized**

```sh
git add src-tauri/src/node_system/runtime/execution_event.rs src-tauri/src/commands src-tauri/src/project/project_state.rs src-tauri/src/lib.rs
GIT_EDITOR=true git commit -m "Expose authoritative results over IPC"
```

---

### Task 7: Frontend Result Contract, Preview, and Presentation Windows

**Files:**
- Create: `src/shared/types/dto/result.ts`
- Create: `src/shared/types/dto/resultParser.ts`
- Create: `src/services/result/resultService.ts`
- Create: `src/services/result/resultService.test.ts`
- Modify: `src/shared/types/dto/runEvent.ts`
- Modify: `src/shared/types/dto/runEventParser.ts`
- Modify: `src/shared/types/dto/runEventParser.test.ts`
- Modify: `src/features/application/editor/observeGraphRunEvent.ts`
- Modify: `src/features/application/editor/requestPinPreview.ts`
- Modify: `src/features/application/editor/observeGraphRunEvent.test.ts`
- Modify: `src/features/application/editor/requestPinPreview.test.ts`
- Modify: `src/features/application/window/openPresentationWindow.ts`
- Modify: `src/features/application/presentation/parsePresentationWindowQuery.ts`
- Modify: `src/features/application/presentation/loadPresentationWindow.ts`
- Modify: `src/features/application/presentation/usePresentationWindow.ts`
- Modify: `src/features/application/window/usePresentationWindowLifecycle.ts`
- Modify: relevant presentation tests

**Interfaces:**
- Consumes: Task 6 exact JSON contracts.
- Produces:

```ts
export type ResultState =
  | { kind: 'pending'; progress: ResultProgress }
  | { kind: 'ready' }
  | { kind: 'failed'; failure: ResultFailure }
  | { kind: 'cancelled' };

export interface ResultDescriptor {
  resultId: string;
  state: ResultState;
  provenance: ResultProvenance;
  presentation: Presentation;
  valueKind: ResultValueKind;
  title: string;
  totalCount?: number;
}

export class ResultService {
  static getDescriptor(resultId: string): Promise<ResultDescriptor | null>;
  static getValue(resultId: string): Promise<ResultValue | null>;
  static getPage(resultId: string, offset: number, limit: number): Promise<ResultPage | null>;
  static getPinHistory(graphPath: string, pinId: string): Promise<PinResultEntry[]>;
}
```

- Presentation route query is `?resultId=...`; there is no release-on-close invoke.

- [ ] **Step 1: Write failing strict parser and service tests**

```ts
it('parses every result state and rejects source and artifact identities', () => {
  expect(parseResultDescriptor(readyResultFixture)).toEqual(readyResultFixture);
  expect(parseResultDescriptor(pendingResultFixture).state.kind).toBe('pending');
  expect(parseResultDescriptor(failedResultFixture).state.kind).toBe('failed');
  expect(() => parseResultDescriptor({ ...readyResultFixture, sourceId: '4' })).toThrow();
  expect(() => parseResultDescriptor({ ...readyResultFixture, artifactId: '9' })).toThrow();
});

it('sends decimal result IDs to exact result commands', async () => {
  await ResultService.getDescriptor('17');
  expect(invoke).toHaveBeenCalledWith('get_result_descriptor', { resultId: '17' });
  expect(invoke).not.toHaveBeenCalledWith(expect.stringContaining('release'), expect.anything());
});

it('loads pending failed cancelled and ready windows by one result ID', async () => {
  await expect(loadPresentationWindow('17')).resolves.toMatchObject({ status: 'pending' });
  await expect(loadPresentationWindow('18')).resolves.toMatchObject({ status: 'failed' });
  await expect(loadPresentationWindow('19')).resolves.toMatchObject({ status: 'cancelled' });
  await expect(loadPresentationWindow('20')).resolves.toMatchObject({ status: 'ready' });
});
```

- [ ] **Step 2: Run frontend tests to verify RED**

Run:

```sh
pnpm test -- src/services/result/resultService.test.ts src/shared/types/dto/runEventParser.test.ts src/features/application/presentation/loadPresentationWindow.test.ts
```

Expected: tests fail because ResultService, ResultId event parsing, and result-state window loading do not exist.

- [ ] **Step 3: Add strict result DTO parsers and service**

Validate exact keys, decimal IDs, state tags, provenance, usage variants, and presentation. Service methods are the only frontend invokes. Do not preserve `SourceService` aliases after callers migrate.

- [ ] **Step 4: Convert run events and Pin Preview to resultId**

Replace the `outputReady` wire variant with the exact Task 6 wire variant `outputResultChanged`. Preserve generation checks but complete preview leases with `resultId`:

```ts
preview.lease.complete(event.kind.resultId);
```

Rename `PinPreviewState.sourceId` and completed request fields to `resultId`. Update strict parser fixtures and reject old `sourceId` payloads.

- [ ] **Step 5: Route and render windows by resultId**

Change presentation URLs to `?resultId=...`. `loadPresentationWindow` first loads the descriptor and returns explicit pending, failed, cancelled, missing, or ready UI state. Fetch value/page only for ready results. Remove `SourceService.releaseResultSource` from `usePresentationWindowLifecycle`; subscription cleanup may remain but result deletion may not.

- [ ] **Step 6: Remove View Data sequence-unwrapping workaround**

Delete the behavior in `sourceValuePayload.ts` that unwraps a one-element report sequence created by View Data. Tests must require the canonical report object directly:

```ts
expect(reportSourceValuePayload({ kind: 'value', value: olsReport })).toEqual(olsReport);
expect(() => reportSourceValuePayload({ kind: 'sequence', value: [olsReport] })).toThrow();
```

- [ ] **Step 7: Run focused frontend validation**

Run:

```sh
pnpm test -- src/services/result/resultService.test.ts src/shared/types/dto/runEventParser.test.ts
pnpm test -- src/features/application/editor/observeGraphRunEvent.test.ts src/features/application/editor/requestPinPreview.test.ts
pnpm test -- src/features/application/presentation/loadPresentationWindow.test.ts src/features/application/presentation/parsePresentationWindowQuery.test.ts src/features/core/resultSource/resolveRenderer.test.ts
pnpm typecheck
```

Expected: all commands exit 0.

- [ ] **Step 8: Commit only if explicitly authorized**

```sh
git add src/shared src/services src/features/application
GIT_EDITOR=true git commit -m "Load result windows by result ID"
```

---

### Task 8: Authoritative Output-Pin History and Pin View

**Files:**
- Modify: `src/features/core/execution/pinViewTarget.ts`
- Modify: `src/features/core/execution/pinResultIndex.ts`
- Modify: `src/features/core/execution/normalizePinResult.ts`
- Modify: `src/features/core/execution/useExecutionStore.ts`
- Modify: `src/shared/types/ui/execution.ts`
- Modify: `src/features/core/resultSource/inspectableSource.ts`
- Modify: `src/features/application/execution/openInspectableSource.ts`
- Test: `src/features/core/execution/resolvePinViewTarget.test.ts`
- Test: `src/features/core/execution/pinResultIndex.test.ts`
- Test: `src/features/core/execution/useExecutionStore.lifecycle.test.ts`
- Test: `src/features/core/resultSource/inspectableSource.test.ts`

**Interfaces:**
- Consumes: `ResultService.getPinHistory`, `PinResultEntry`, exact upstream connection resolution, result window opening from Task 7.
- Produces:

```ts
export type InspectableResultRef =
  | { kind: 'result'; resultId: string }
  | { kind: 'outputPin'; graphPath: string; pinId: string };

export interface PinHistoryProjection {
  graphPath: string;
  outputPinId: string;
  entries: PinResultEntry[];
  selectedResultId: string | null;
}
```

- [ ] **Step 1: Write failing history-selection and connection tests**

```ts
it('selects the latest occurrence even when it failed', async () => {
  mockPinHistory([
    readyEntry('17'),
    failedEntry('18'),
  ]);
  await expect(resolveOutputPinView(outputPin)).resolves.toEqual({
    kind: 'result',
    resultId: '18',
  });
});

it('resolves an input pin only through its connected upstream output history', async () => {
  await resolveInputPinView(inputPinConnectedTo(outputPin));
  expect(ResultService.getPinHistory).toHaveBeenCalledWith(graphPath, outputPin.id);
  expect(ResultService.getPinHistory).not.toHaveBeenCalledWith(graphPath, inputPin.id);
});

it('never falls back by pin ID across graph paths', () => {
  expect(lookupPinHistory(state, 'events/a.yssbi-event', sharedPinId)).not.toEqual(
    lookupPinHistory(state, 'events/b.yssbi-event', sharedPinId),
  );
});
```

Also test opening pending and cancelled entries, historical selection, and graph-tab close clearing only frontend projections.

- [ ] **Step 2: Run focused tests to verify RED**

Run:

```sh
pnpm test -- src/features/core/execution/resolvePinViewTarget.test.ts src/features/core/execution/pinResultIndex.test.ts src/features/core/execution/useExecutionStore.lifecycle.test.ts src/features/core/resultSource/inspectableSource.test.ts
```

Expected: failures because Pin View still uses `sourceId`, a disabled runtime fallback, and pin-ID-only lookup.

- [ ] **Step 3: Replace source references with exact result/output-Pin references**

Use `ResultService.getPinHistory` for an output Pin. For an input Pin, resolve the graph connection first and query the upstream output. Default to `entries.at(-1)` regardless of state. Add explicit history selection that opens the chosen `resultId`.

- [ ] **Step 4: Remove ambiguous and ownership-coupled caches**

Delete `lookupPinResult`'s pin-ID-only fallback. Key projections by opaque `graphPath + outputPinId`. Rename `clearGraphRunArtifacts` to projection-only language or remove it; it must not invoke backend result deletion. Tab close clears UI projections only.

- [ ] **Step 5: Run Pin View validation**

Run:

```sh
pnpm test -- src/features/core/execution/resolvePinViewTarget.test.ts src/features/core/execution/pinResultIndex.test.ts src/features/core/execution/useExecutionStore.lifecycle.test.ts src/features/core/resultSource/inspectableSource.test.ts
pnpm typecheck
```

Expected: all commands exit 0.

- [ ] **Step 6: Commit only if explicitly authorized**

```sh
git add src/features/core/execution src/features/core/resultSource src/features/application/execution src/shared/types/ui
GIT_EDITOR=true git commit -m "Open Pin history by result ID"
```

---

### Task 9: Remove View Data Snapshot and Open the Exact Input Result

**Files:**
- Modify: `src-tauri/src/node_system/catalog/core_nodes/debug.rs`
- Modify: `src-tauri/src/node_system/catalog/core_nodes/support.rs`
- Modify: `src-tauri/src/node_system/runtime/kernels/core_nodes/debug.rs`
- Modify: `src-tauri/src/node_system/runtime/scheduler.rs`
- Modify: `src-tauri/src/node_system/runtime/builtin_tests.rs`
- Modify: `src-tauri/src/node_system/catalog/core_nodes/coverage_tests.rs`
- Modify: `src/features/application/editor/useProjectOperations.ts`
- Modify: `src/features/application/editor/useProjectOperations.execution.test.tsx`

**Interfaces:**
- Consumes: Frame's exact input `ResultId`, Task 6 `OpenResultWindow`, existing backend `NOTIFY` logger, Task 7 result-window opening.
- Produces: View Data catalog shape `Enter -> View Data -> Then` plus `Data` input, and scheduler side effect `OpenResultWindow { result_id }`.

- [ ] **Step 1: Write failing catalog, runtime, and frontend tests**

```rust
#[test]
fn view_data_has_no_data_output_or_fragment_result() {
    let definition = production_catalog().node("yssbi.debug.view").unwrap();
    assert_eq!(definition.data_inputs().map(|port| port.key()).collect::<Vec<_>>(), ["data"]);
    assert!(definition.data_outputs().next().is_none());
    assert!(!definition.lowering().has_fragment_result());
}

#[test]
fn view_data_opens_exact_input_result_without_materialization() {
    let fixture = ViewDataResultFixture::run();
    assert_eq!(fixture.input_result_id(), fixture.open_window_result_id());
    assert_eq!(fixture.result_count_before_view(), fixture.result_count_after_view());
    assert_eq!(fixture.materialization_count(), 0);
}
```

Update the frontend ordinary execution test:

```ts
it('opens View Data from the backend exact-result request', async () => {
  emitRunEvent({ type: 'openResultWindow', resultId: '42' });
  expect(openResultWindow).toHaveBeenCalledWith('42');
  expect(openResultWindow).toHaveBeenCalledTimes(1);
});
```

Assert unrelated output-result and Pin Preview events do not open windows.

- [ ] **Step 2: Run focused tests to verify RED**

Run:

```sh
pnpm rust:test --lib view_data_has_no_data_output_or_fragment_result
pnpm rust:test --lib view_data_opens_exact_input_result_without_materialization
pnpm test -- src/features/application/editor/useProjectOperations.execution.test.tsx
```

Expected: failures because View Data still owns `snapshot`, replay materialization, and frontend node-type detection.

- [ ] **Step 3: Simplify the View Data catalog**

In `register_view`, remove `snapshot`, replay/snapshot descriptions, and `result_leaf`. Register the existing effectful leaf with only `enter`, `data`, and `then`. Do not delete `result_leaf` globally because other result-producing nodes use it.

- [ ] **Step 4: Move View Data to a scheduler side effect**

Remove `ViewKernel` cursor reads, `materialize_artifact`, and replay output construction. When scheduling the View Data operation, resolve its Data input's exact Frame binding and, after the effect activation succeeds, emit:

```rust
RunEventKind::OpenResultWindow { result_id: input_result_id }
```

Write one backend `NOTIFY` log containing result ID, run ID, activation ID, and View Data node ID. Do not expose ResultStore or Tauri window APIs to ordinary kernels.

- [ ] **Step 5: Open only from the explicit frontend event**

In `useProjectOperations`, remove node lookup and `nodeType === 'yssbi.debug.view'` checks against ordinary output events. Handle `openResultWindow` directly and pass its `resultId` to the Task 7 window opener.

- [ ] **Step 6: Run View Data validation**

Run:

```sh
pnpm rust:test --lib do_sleep_print_and_view_leaf_kernels_preserve_contracts
pnpm rust:test --lib view_data_has_no_data_output_or_fragment_result
pnpm rust:test --lib view_data_opens_exact_input_result_without_materialization
pnpm test -- src/features/application/editor/useProjectOperations.execution.test.tsx
pnpm typecheck
pnpm rust:fmt:check
pnpm rust:check
```

Expected: all commands exit 0.

- [ ] **Step 7: Commit only if explicitly authorized**

```sh
git add src-tauri/src/node_system/catalog/core_nodes src-tauri/src/node_system/runtime src/features/application/editor
GIT_EDITOR=true git commit -m "Open View Data from its input result"
```

---

### Task 10: Delete Legacy Artifact/ResultSource Authority and Prove OLS End to End

**Files:**
- Delete: `src-tauri/src/node_system/runtime/artifact.rs`
- Modify: `src-tauri/src/node_system/runtime/mod.rs`
- Modify: `src-tauri/src/node_system/runtime/run.rs`
- Modify: `src-tauri/src/node_system/runtime/result_store.rs`
- Modify: `src-tauri/src/node_system/runtime/scheduler.rs`
- Modify: `src-tauri/src/commands/command_node_system.rs`
- Modify: `src-tauri/src/commands/node_system_execution_dto.rs`
- Modify: `src-tauri/src/lib.rs`
- Delete: `src/shared/types/dto/resultSource.ts`
- Delete: `src/services/resultSource/resultSourceService.ts`
- Delete or rename: obsolete files under `src/features/core/resultSource/` after all imports use result terminology
- Modify: `src-tauri/src/project/production_tests.rs`
- Modify: report validation/rendering files under `src/features/core/resultSource/` and `src/views/InfoView/`

**Interfaces:**
- Consumes: all prior tasks.
- Produces: only `Frame -> ResultId -> ResultStore -> StoredResult -> StoredValue`, plus lightweight Pin-history and memo indexes.

- [ ] **Step 1: Rewrite the real external regression before deleting compatibility code**

Replace the snapshot/source assertions in `external_project_event_executes_when_diagnostic_path_is_configured` with exact identity assertions:

```rust
assert_eq!(ols_report_result_id, view_data_input_result_id);
assert_eq!(view_data_input_result_id, open_window_result_id);
assert_eq!(open_window_result_id, latest_ols_report_pin_history.result_id);
assert_eq!(result_store.result_count_for_node(view_data_node_id), 0);

let report = result_store.result(ols_report_result_id).unwrap();
assert_eq!(report.presentation, ResultPresentation::Report { report: ResultReportKind::OlsSummary });
assert_eq!(canonical_report_title(&report), Some("OLS Summary"));
```

Also add a malformed OLS report renderer test that requires `ResultId`, producer provenance, actual value category, and exact field path in the diagnostic.

- [ ] **Step 2: Run the regression to establish its pre-deletion state**

Run the focused in-repository test first:

```sh
pnpm rust:test --lib external_project_event_executes_when_diagnostic_path_is_configured -- --test-threads=1
```

Expected without environment configuration: test exits 0 after its documented skip. For the real graph, configure these literal values in the invoking shell and run the same command:

```text
YSSBI_DIAGNOSTIC_PROJECT_ROOT=C:\Users\zhou.yi31\Documents\New Project
YSSBI_DIAGNOSTIC_GRAPH_PATH=events/New Event.yssbi-event
```

Expected with configuration: the Default-demand run passes all same-`ResultId`, OLS title, and report-presentation assertions.

- [ ] **Step 3: Delete old Rust business identities and lifecycle paths**

Remove:

```text
ArtifactId
ArtifactSnapshot
ArtifactSnapshotKind
ArtifactDescriptor
ArtifactPage
ArtifactStore
ArtifactPublicationGuard
result_source_holds
run-owned artifact result lifecycle
ResultSourceId as data identity
PendingResultSource
prepare_runtime_value*
publish_runtime_value
stage_result_sources
commit_result_sources
release_result_source
release_run_result_sources
ArtifactKind::Replayable
RuntimeValue::Artifact as Frame/result identity
```

If `RuntimeValue` remains at the kernel call boundary, rename it to `KernelValue` and ensure searches show it is absent from Frame, memoization completion, IPC DTOs, and StoredResult.

- [ ] **Step 4: Delete old frontend source ownership and stale compatibility paths**

Remove old source DTO/service files, release-on-close behavior, `sourceId` result fields, `artifactId`, View Data sequence unwrapping, and stale disconnected `pinResultReady/openSourceWindow` compatibility code after confirming no active producer exists. Do not turn `clear_graph_execution_artifacts` into result deletion; remove the unregistered stale call instead.

- [ ] **Step 5: Add actionable report validation diagnostics**

Validate only a ready canonical report value. Produce a structured diagnostic such as:

```ts
{
  resultId,
  runId,
  activationId,
  nodeId,
  outputPinId,
  presentation: { kind: 'report', report: 'olsSummary' },
  valueKind,
  fieldPath: 'coefficients[0].std_error',
  reason: 'missing required field',
}
```

Show a concise error in the window and send the full diagnostic to `NOTIFY` or `APP` log. Do not retain the generic-only “报告数据格式无效，无法渲染” path.

- [ ] **Step 6: Prove obsolete identities are absent**

Run repository searches and require no production matches:

```sh
pnpm exec node -e "const fs=require('fs');const path=require('path');const roots=['src','src-tauri/src'];const banned=/ArtifactSnapshot|ArtifactStore|ArtifactId|ResultSourceId|sourceId|artifactId|Replayable/;const walk=p=>fs.readdirSync(p,{withFileTypes:true}).flatMap(e=>e.isDirectory()?walk(path.join(p,e.name)):[[path.join(p,e.name),fs.readFileSync(path.join(p,e.name),'utf8')]]);const hits=roots.flatMap(walk).filter(([p,s])=>banned.test(s));if(hits.length){console.error(hits.map(([p])=>p).join('\n'));process.exit(1)}"
```

Expected: exit 0 with no output. If a term is retained for an unrelated domain, narrow the script to node-system result/presentation files and document that exact unrelated use in the implementation notes; do not preserve result compatibility code.

- [ ] **Step 7: Run focused Rust and frontend regression suites**

Run:

```sh
pnpm rust:test --lib node_system::runtime::result_store::tests
pnpm rust:test --lib kernel_receives_all_inputs_once_and_outputs_publish_atomically
pnpm rust:test --lib spill_backed_stored_value_supports_two_independent_passes
pnpm rust:test --lib memoization_reuses_ordered_output_result_ids_and_history
pnpm rust:test --lib view_data_opens_exact_input_result_without_materialization
pnpm rust:test --lib external_project_event_executes_when_diagnostic_path_is_configured -- --test-threads=1
pnpm test -- src/services/result/resultService.test.ts src/shared/types/dto/runEventParser.test.ts
pnpm test -- src/features/application/editor/useProjectOperations.execution.test.tsx src/features/application/editor/observeGraphRunEvent.test.ts
pnpm test -- src/features/core/execution/resolvePinViewTarget.test.ts src/features/core/execution/pinResultIndex.test.ts
pnpm test -- src/features/application/presentation/loadPresentationWindow.test.ts src/features/core/resultSource/resolveRenderer.test.ts
```

Expected: every command exits 0.

- [ ] **Step 8: Run final cross-stack verification**

Run:

```sh
pnpm verify
git diff --check
```

Expected: both exit 0. Report existing unrelated warnings separately; do not change unrelated code to silence them.

- [ ] **Step 9: Commit only if explicitly authorized**

```sh
git add -A src src-tauri
GIT_EDITOR=true git commit -m "Unify execution results in ResultStore"
```

Otherwise leave the complete verified migration uncommitted for user review.

---

## Execution Notes

- Execute tasks in order. Task 3 is the identity cutover and must not begin until Tasks 1–2 are green.
- Keep the old Artifact implementation only as Task 2's private physical bridge. New Frame, Pin, event, command, service, or window code must never accept an Artifact or ResultSource identity.
- Do not combine Task 3 scheduler semantics with Task 9 View Data removal in one unreviewed change; View Data should be simplified only after exact input ResultIds can cross the event/IPC boundary.
- Preserve the current cancellation, deadline, retry, workload admission, bounded-channel, spill-quota, memory-budget, and project-drain tests throughout the migration.
- When a task exposes an existing unrelated failure, record it and continue only if the changed task's focused tests prove the failure was pre-existing.
- The final external regression must use `ExecutionDemand::Default`; PinPreview alone does not cover View Data's ordinary execution path.
