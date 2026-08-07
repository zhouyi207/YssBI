# Node Architecture TODO Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close every unchecked functional item in `TODO.md:293-316` and make canonical `pnpm verify` pass without the Windows `LNK1102` exception.

**Architecture:** Deliver vertical authority slices in dependency order. Rust remains authoritative for project, graph, compiler, execution, and trace state; TypeScript strictly parses and projects wire results. Execution work proceeds from explicit demand and plan metadata into run-owned cache, materialization, scheduler, deadline, retry, and trace resources.

**Tech Stack:** Rust 2024, Tauri 2, serde/syn, React 19, TypeScript 5.8, Zustand, Vitest 4, pnpm 11, `pinyin-pro@3.28.2`.

## Global Constraints

- Work in place on the existing `shadcn` branch; preserve commit `f9fe4aa0` and all unrelated user changes.
- Do not create commits, staging, branches, worktrees, tags, merges, or pushes.
- `ProjectState.project_data` remains authoritative; commands stay thin.
- Do not hold global locks during I/O, channel waits, retry sleeps, model loading, or operation execution.
- Frontend services own IPC parsing; stale lifecycle completion has zero side effects.
- Do not add compatibility exports, deprecated aliases, optional identity overloads, wildcard exemptions, or dual mutation paths.
- Use TypeScript `Program`/`TypeChecker` for semantic frontend audits and `syn` for Rust source audits.
- Run Cargo through root `pnpm` scripts; all Rust test scripts use `--jobs 1` after Task 1.
- Each behavior task follows observed RED → minimal GREEN → focused regression → specification review → quality review.
- Only check a `TODO.md` item after its complete vertical slice and focused verification pass.

---

### Task 1: Stabilize the canonical Windows Rust verification workflow

**Files:**
- Modify: `package.json:15-23`
- Modify: `docs/development/LOCAL_WORKFLOW.md:21-42`
- Test: `src-tauri/tests/sci_api_bayes_linear_normal_golden_test.rs`
- Test: `src-tauri/tests/sci_api_bayes_validation_golden_test.rs`
- Test: `src-tauri/tests/sci_api_time_series_acf_pacf_golden_test.rs`
- Test: `src-tauri/tests/sci_api_time_series_serial_tests_golden_test.rs`

**Interfaces:**
- Produces: cross-platform `pnpm rust:test` and `pnpm rust:test:sci` scripts that pass Cargo `--jobs 1`.
- Preserves: command arguments following `pnpm rust:test -- ...` and the root `target/` directory.

- [ ] **Step 1: Record the existing failing-environment evidence**

Document in the task report that the normal `pnpm verify` failed while linking the four named binaries with `LNK1102`, while all four passed 12/12 under one Cargo job. Do not manufacture another OOM as RED.

- [ ] **Step 2: Add cross-platform Cargo job bounds**

Change the scripts to:

```json
"rust:test": "cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --jobs 1",
"rust:test:sci": "cargo test --manifest-path src-tauri/Cargo.toml -p yss-sci --jobs 1"
```

Do not use `CARGO_BUILD_JOBS=1` in `package.json` and do not globally set `[build] jobs` in `.cargo/config.toml`.

- [ ] **Step 3: Update the canonical workflow documentation**

State that Rust test linking is serialized to avoid Windows linker-memory spikes, while `rust:check` and development builds retain normal Cargo parallelism.

- [ ] **Step 4: Verify the four prior failures**

Run:

```text
pnpm rust:test --test sci_api_bayes_linear_normal_golden_test --test sci_api_bayes_validation_golden_test --test sci_api_time_series_acf_pacf_golden_test --test sci_api_time_series_serial_tests_golden_test -- --test-threads=1
```

Expected: PASS, 4 binaries / 12 tests; no `LNK1102`.

- [ ] **Step 5: Verify the Rust workflow**

Run: `pnpm verify:rust`

Expected: exit 0. Record existing warnings exactly; do not relabel warnings as failures.

---

### Task 2: Add the Rust-authoritative function editor projection wire

**Files:**
- Modify: `src-tauri/src/node_system/analysis/projection.rs`
- Modify: `src-tauri/src/project/project_io.rs`
- Modify: `src-tauri/src/project/project_reads.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/event/event_project.rs`
- Modify: `src-tauri/src/node_system/testing/contracts.rs`
- Modify: `src/shared/types/dto/editorProjection.ts`
- Modify: `src/services/project/projectService.ts`
- Test: `src/services/project/projectService.test.ts`
- Test: `src/services/nodeSystem/nodeSystemGoldenContracts.test.ts`

**Interfaces:**
- Produces Rust/TS wire:

```rust
pub struct FunctionEditorPinDto {
    pub id: Box<str>,
    pub name: Box<str>,
    pub data_type: DataType,
}

pub struct FunctionEditorProjectionDto {
    pub function_revision: ResourceRevision,
    pub inputs: Box<[FunctionEditorPinDto]>,
    pub outputs: Box<[FunctionEditorPinDto]>,
}
```

```ts
export interface FunctionEditorProjectionDto {
  functionRevision: number
  inputs: FunctionSignaturePin[]
  outputs: FunctionSignaturePin[]
}
```

- `ProjectGraphIndexEntry` produces `function_editor_projection: Option<FunctionEditorProjectionDto>` for function graphs.
- Function mutation projection replacements carry the same resolved pin data; no frontend type inference is allowed.

- [ ] **Step 1: Write Rust golden RED**

Add a function fixture whose resolved output name/type differ from the old frontend defaults. Assert serialized project index and project-event replacement contain exact IDs, names, and structured data types.

Run:

```text
pnpm rust:test --lib function_editor_projection_wire -- --test-threads=1
```

Expected RED: `functionEditorProjection` is absent.

- [ ] **Step 2: Write TypeScript strict-parser RED**

Add cases that reject missing `inputs`, malformed structured data types, and legacy rows containing only raw `functionSignature`. Assert a non-`Result` output name remains unchanged.

Run:

```text
pnpm test src/services/project/projectService.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts
```

Expected RED: new wire cannot be parsed.

- [ ] **Step 3: Implement authoritative projection construction**

Resolve pins in Rust from the normalized function document/registry types and attach the DTO to project-index rows and replacement projections. The constructor must be shared by load, mutation publication, and recovery sources.

- [ ] **Step 4: Implement strict TypeScript parsing**

Parse exact keys and structured data types before returning `ProjectGraphIndexRow`; reject unknown fallback types rather than returning `{ kind: 'Any' }`.

- [ ] **Step 5: Run focused GREEN**

Run both RED commands. Expected: PASS with all golden fixtures byte/shape aligned.

---

### Task 3: Remove frontend function-signature business projection

**Files:**
- Modify: `src/features/application/graphDocument/functionSignatureSync.ts`
- Modify: `src/features/core/dataStore/authoritativeProjectLoadPlan.ts`
- Modify: `src/features/application/editorMutation/resourceMutationResult.ts`
- Modify: `src/features/application/editorMutation/projectPublicationRecovery.ts`
- Modify: `src/features/application/editorMutation/functionSignatureCoordinator.ts`
- Modify: `src/features/core/dataStore/graphMetaStore.ts`
- Test: `src/features/application/graphDocument/functionSignatureSync.test.ts`
- Test: `src/features/application/editorMutation/functionSignatureCoordinator.test.ts`
- Test: `src/features/application/editorMutation/projectPublicationRecovery.test.ts`
- Test: `src/features/core/dataStore/projectIOStore.test.ts`
- Test: `src/services/nodeSystem/nodeIdentityArchitectureContract.test.ts`

**Interfaces:**
- Consumes: `FunctionEditorProjectionDto` from Task 2.
- Produces:

```ts
export function installFunctionEditorProjection(
  graphPath: string,
  signature: FunctionSignatureDto,
  projection: FunctionEditorProjectionDto,
): PreparedFunctionDeltaInstall
```

- Removes: `functionSignaturePins` and all production `dataTypeFromDisplayString` use for function pins.

- [ ] **Step 1: Replace old behavior tests with authoritative-wire RED**

Test load, publication, and recovery using backend-provided output name `Computed value` and a structured type that the old display parser would map incorrectly. Expect exact installation.

- [ ] **Step 2: Add semantic architecture RED**

Extend the TypeChecker audit to reject production imports/calls of `functionSignaturePins`, signature-to-pin mapping, fixed `Result`, and fallback `Any` in function interface projection.

Run:

```text
pnpm test src/services/nodeSystem/nodeIdentityArchitectureContract.test.ts src/features/application/graphDocument/functionSignatureSync.test.ts
```

Expected RED: production mapping symbols are found.

- [ ] **Step 3: Replace every reconstruction path**

Load, publication, conflict hydration, and recovery must copy `projection.inputs` and `projection.outputs` directly into `GraphMeta`. Keep raw signature only as editable authority metadata.

- [ ] **Step 4: Delete the projector**

Remove `functionSignaturePins`, its imports, and tests asserting display-string conversion or fixed names.

- [ ] **Step 5: Run focused and type verification**

Run:

```text
pnpm test src/features/application/graphDocument/functionSignatureSync.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/application/editorMutation/projectPublicationRecovery.test.ts src/features/core/dataStore/projectIOStore.test.ts src/services/nodeSystem/nodeIdentityArchitectureContract.test.ts
pnpm typecheck
```

Expected: PASS.

---

### Task 4: Make graph resource moves replacement-only in the frontend

**Files:**
- Modify: `src-tauri/src/project/resource_mutations.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src/features/application/editorMutation/projectPublicationMovePlan.ts`
- Modify: `src/features/application/editorMutation/resourceMutationResult.ts`
- Modify: `src/features/application/editor/cascadeGraphPathReferences.ts`
- Test: `src/features/application/editorMutation/projectPublicationMovePlan.test.ts`
- Test: `src/features/application/editorMutation/projectPublicationProductionStores.test.ts`
- Test: `src/features/application/editorMutation/resourceMutationResult.test.ts`
- Test: `src-tauri/src/project/resource_mutations.rs`
- Test: `src/services/nodeSystem/nodeIdentityArchitectureContract.test.ts`

**Interfaces:**
- Consumes: `ResourceMutationResultDto.projectionReplacements` for every loaded affected caller graph.
- Produces: `prepareGraphResourceMove` that migrates only path-keyed UI ownership; graph nodes change only through `commitPreparedGraphProjectionReplacements`.
- Removes: `prepareReferences`, `referenceSnapshot`, `commitReferenceSnapshot`, `cascadeSubGraphPathInLoadedGraphs`, and `cascadeGraphPathReferences`.

- [ ] **Step 1: Write Rust caller-replacement RED**

Assert a rename result contains destination and loaded caller replacements, each with source revision equal to its committed graph revision and with the caller target already remapped.

- [ ] **Step 2: Write frontend authority RED**

Provide a replacement intentionally different from what scanning old `subGraphPath` could infer. Assert commit installs the replacement exactly. If complete status names a caller without a replacement, assert protocol failure/recovery and no local node mutation.

- [ ] **Step 3: Add architecture RED**

Reject production use of the removed symbols and direct `useGraphDataStore.setState` node mutation from editor mutation/cascade modules.

- [ ] **Step 4: Remove local reference mutation**

Delete snapshots/scans and route all graph changes through prepared revisioned replacements. Retain only tab, focus, viewport, selection, and persisted editor-view remapping.

- [ ] **Step 5: Run focused GREEN**

Run:

```text
pnpm test src/features/application/editorMutation/projectPublicationMovePlan.test.ts src/features/application/editorMutation/projectPublicationProductionStores.test.ts src/features/application/editorMutation/resourceMutationResult.test.ts src/services/nodeSystem/nodeIdentityArchitectureContract.test.ts
pnpm rust:test --lib loaded_caller_rename_cascade_survives_fresh_reload -- --test-threads=1
```

Expected: PASS.

---

### Task 5: Remove legacy node creation and identity DTOs

**Files:**
- Delete: `src/shared/types/dto/batchCreateNode.ts`
- Delete: `src/shared/types/dto/batchCreateNode.test.ts`
- Delete: `src/shared/types/dto/nodeInstanceParams.ts`
- Delete: `src/shared/types/dto/nodeInstanceParams.test.ts`
- Modify: `src/features/application/dataManagement/useNodeManagement.ts`
- Modify: `src/features/application/editor/editorSessionTypes.ts`
- Modify: `src/features/core/editor/stores/useClipboardStore.ts`
- Modify: `src/features/core/editor/clipboardSnapshot.ts`
- Modify: `src/shared/types/dto/graph.ts`
- Modify: `src/shared/types/dto/graphModel.ts`
- Modify: `src/shared/types/dto/graphConverters.ts`
- Modify: `src/shared/types/store/graph.ts`
- Modify: `src/services/project/projectService.ts`
- Test: `src/services/nodeSystem/nodeIdentityArchitectureContract.test.ts`
- Test: `src/services/nodeSystem/graphLoadContract.test.ts`
- Test: `src/features/application/editor/useEditorOperations.capabilities.test.tsx`

**Interfaces:**
- Preserves: descriptor-backed `createNode(NodeCreationDescriptor)`.
- Removes: `createNodes`, `BatchCreateNodeRequest`, `BatchCreateNodeIpcItem`, `NodeInstanceParamsDTO`, `NodeSpawnParams`, `ParamsKind`, `GraphInstanceDTO`, `NodeInstanceDTO`, and clipboard `params`.
- Clipboard stores stable projection identity only; it does not synthesize a replayable creation descriptor from display fields.

- [ ] **Step 1: Add semantic architecture RED**

Audit imports, aliases, namespace access, re-exports, declarations, and production property access for every removed symbol/module.

- [ ] **Step 2: Add clipboard/session RED**

Assert `EditorSessionNodeActions` has only current descriptor-backed creation/deletion APIs and clipboard snapshots omit legacy params/display-name identity.

- [ ] **Step 3: Delete dead API and DTO paths**

Remove files, exports, hook members, session picker members, graph conversion compatibility unions, and obsolete tests. Do not add replacement aliases.

- [ ] **Step 4: Run focused GREEN**

Run:

```text
pnpm test src/services/nodeSystem/nodeIdentityArchitectureContract.test.ts src/services/nodeSystem/graphLoadContract.test.ts src/features/application/editor/useEditorOperations.capabilities.test.tsx
pnpm typecheck
```

Expected: PASS and no removed-symbol diagnostics.

---

### Task 6: Restrict raw `GraphDocument` mutation methods to tests

**Files:**
- Modify: `src-tauri/src/node_system/document/transaction.rs`
- Modify: `src-tauri/src/node_system/document/mutation.rs`
- Modify: `src-tauri/src/node_system/testing/source_audit.rs`
- Test: `src-tauri/src/node_system/document/tests.rs`
- Test: `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs`

**Interfaces:**
- Production mutation remains `EditorGraphMutationDto::into_patch_with_catalog_snapshot(...) -> Result<GraphDocumentPatch, MutationConflict>` followed by authoritative atomic patch commit.
- Raw `create_node`, `delete_node`, `bind_port`, `connect`, `disconnect`, and `set_literal` exist only under `#[cfg(test)]` with `pub(crate)` visibility.

- [ ] **Step 1: Add syn-based visibility RED**

Parse `transaction.rs` and assert all six methods are absent from the production impl. Scan production scopes for method calls, UFCS references, aliases, and cfg bypasses.

- [ ] **Step 2: Gate raw helpers**

Move the six helpers into a strict `#[cfg(test)] impl GraphDocument` and retain production read/validation methods separately.

- [ ] **Step 3: Verify descriptor authority and tests**

Run:

```text
pnpm rust:test --lib production_graph_document_exposes_no_raw_mutation_methods -- --test-threads=1
pnpm rust:test --lib node_system::document::tests -- --test-threads=1
pnpm rust:check
```

Expected: PASS.

---

### Task 7: Track exact resources read by Analysis

**Files:**
- Modify: `src-tauri/src/node_system/analysis/basis.rs`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs`
- Modify: `src-tauri/src/node_system/compiler/project.rs`
- Modify: `src-tauri/src/node_system/compiler/coordinator.rs`
- Modify: `src-tauri/src/project/compile_publication.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Test: `src-tauri/src/node_system/compiler/tests.rs`
- Test: `src-tauri/src/project/production_tests.rs`

**Interfaces:**
- Produces:

```rust
pub struct ResolvedResource<T> {
    pub key: ResourceKey,
    pub version: ResourceVersion,
    pub value: T,
}

pub trait AnalysisResourceResolver {
    fn resolve_function(&mut self, path: &GraphResourcePath) -> Result<ResolvedFunction<'_>, ResourceResolutionError>;
    fn resolve_variable(&mut self, id: &VariableId) -> Result<ResolvedVariable<'_>, ResourceResolutionError>;
    fn resolve_database(&mut self, id: &DatabaseId) -> Result<ResolvedDatabase<'_>, ResourceResolutionError>;
    fn reads(&self) -> &AnalysisResourceReads;
}
```

- `CompilationBasis.resource_versions` contains only successful reads.
- Freshness compares graph revision, registry fingerprint, lifecycle authority, and current versions of exact keys only.

- [ ] **Step 1: Add compiler RED matrix**

Cover no-resource graph → empty set; one used/unrelated function; one used/unrelated variable; one used/unrelated database. Duplicate reads record once.

- [ ] **Step 2: Add publication freshness RED**

Compile, mutate unrelated resource, and assert same published compile ID remains current. Mutate actual dependency and assert recompilation for function, variable, and database cases.

- [ ] **Step 3: Implement coherent tracked resolver**

Value and version must come from the same project snapshot. Database schema analysis and variable nodes must use the resolver rather than independent maps.

- [ ] **Step 4: Separate request identity from final basis**

Do not preload all resource versions into the compilation task. Publish only if lifecycle/graph/registry and each exact read remain current.

- [ ] **Step 5: Run focused GREEN**

Run:

```text
pnpm rust:test --lib compilation_basis_contains_only_resources_read_by_analysis -- --test-threads=1
pnpm rust:test --lib unrelated_resource_mutation_preserves_published_compilation -- --test-threads=1
pnpm rust:check
```

Expected: PASS.

---

### Task 8: Move user-fixable lowerability failures into Analysis

**Files:**
- Create: `src-tauri/src/node_system/protocol/validation.rs`
- Modify: `src-tauri/src/node_system/protocol/mod.rs`
- Modify: `src-tauri/src/node_system/document/mutation.rs`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs`
- Modify: `src-tauri/src/node_system/compiler/lowering.rs`
- Modify: `src-tauri/src/node_system/catalog/dataframe/mod.rs`
- Test: `src-tauri/src/node_system/compiler/tests.rs`
- Test: `src-tauri/src/node_system/compiler/task1_tests.rs`
- Test: `src-tauri/src/node_system/catalog/dataframe/tests.rs`

**Interfaces:**
- Produces shared pure validation:

```rust
pub fn validate_parameter_values(
    protocol: &NodeProtocol,
    values: &ParameterValues,
    nominal: &impl NominalParameterValidator,
) -> Vec<LocatedParameterIssue>
```

- Analysis validates primitive type/constraints, nominal codecs, resource identity/existence, typed literal decoding, call ABI availability, and implementation-specific lowerability before sealing `ValidatedSemanticGraph`.
- `NodeLowerer` consumes validated configuration and returns only cancellation, deadline/resource exhaustion, or internal invariant errors.

- [ ] **Step 1: Add Analysis RED cases**

Cover invalid dataframe limit range/type, rename parameter type, malformed resource ID, malformed persisted literal, missing callee, and blocking callee. Assert precise locations, no semantic/plan, no lowering-start trace, and zero lowerer calls.

- [ ] **Step 2: Extract shared parameter validation**

Map the same `LocatedParameterIssue` into editor `MutationConflict` and compiler diagnostics; remove duplicated primitive/constraint parsing.

- [ ] **Step 3: Validate all lowerability before semantic seal**

Decode typed literals and prepared node configuration once. Function ABI resolution uses Task 7's tracked resolver and records dependencies.

- [ ] **Step 4: Narrow lowering errors**

Replace arbitrary user-facing `LoweringError.message` handling with typed internal/runtime failures. Do not map arbitrary lowerer strings to `compiler.lowering.failed`.

- [ ] **Step 5: Run focused GREEN**

Run:

```text
pnpm rust:test --lib lowerability -- --test-threads=1
pnpm rust:test --lib node_system::catalog::dataframe::tests -- --test-threads=1
pnpm rust:check
```

Expected: PASS; user-fixable fixtures emit Analysis diagnostics only.

---

### Task 9: Publish only planned demands and add independent Pin preview demand

**Files:**
- Modify: `src-tauri/src/node_system/plan/model.rs`
- Modify: `src-tauri/src/node_system/compiler/specialization/mod.rs`
- Modify: `src-tauri/src/node_system/compiler/specialization/finalization.rs`
- Modify: `src-tauri/src/node_system/runtime/execution_event.rs`
- Modify: `src-tauri/src/node_system/runtime/scheduler.rs`
- Modify: `src-tauri/src/commands/node_system_execution_dto.rs`
- Modify: `src/shared/types/dto/executionDemand.ts`
- Modify: `src/shared/types/dto/runEvent.ts`
- Modify: `src/shared/types/dto/runEventParser.ts`
- Modify: `src/features/application/editor/requestPinPreview.ts`
- Test: `src-tauri/src/node_system/runtime/tests.rs`
- Test: `src-tauri/src/project/production_tests.rs`
- Test: `src/shared/types/dto/executionDemand.test.ts`
- Test: `src/shared/types/dto/runEventParser.test.ts`
- Test: `src/features/application/editor/requestPinPreview.test.ts`
- Test: `src/services/nodeSystem/nodeSystemGoldenContracts.test.ts`

**Interfaces:**
- Produces:

```rust
pub enum PlannedPublication {
    GraphResult { name: Box<str>, output: GraphOutputRef, value: ValueRef },
    PinPreview { output: GraphOutputRef, generation: u64, value: ValueRef },
}
```

- `ExecutionPlan.publications: Box<[PlannedPublication]>` is the only result-source publication authority.
- Removes `ValueReady` from Rust/TS/golden wire.
- Pin preview demand carries generation and never implies default results.

- [ ] **Step 1: Add intermediate-publication RED**

Run a two-operation chain requesting only the second output. Assert one source, one target event, no `ValueReady`, and no readable intermediate source.

- [ ] **Step 2: Add preview identity RED**

Assert ordinary explicit output and preview produce different normalized selection digests; stale generation cannot settle another preview.

- [ ] **Step 3: Make publications explicit**

Finalize selected outputs into `PlannedPublication`; delete per-operation output source staging and `ValueReady` variants/parsers/fixtures.

- [ ] **Step 4: Route preview through the dedicated wire**

Frontend sends preview generation in the demand. Rust returns only the requested preview publication; existing project/session/run guards remain.

- [ ] **Step 5: Run focused GREEN and golden checks**

Run:

```text
pnpm rust:test --lib demand_driven_publication -- --test-threads=1
pnpm test src/shared/types/dto/executionDemand.test.ts src/shared/types/dto/runEventParser.test.ts src/features/application/editor/requestPinPreview.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts
pnpm typecheck
```

Expected: PASS.

---

### Task 10: Carry effective `CachePolicy` into execution plans

**Files:**
- Modify: `src-tauri/src/node_system/protocol/model.rs`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs`
- Modify: `src-tauri/src/node_system/compiler/specialization/mod.rs`
- Modify: `src-tauri/src/node_system/compiler/specialization/finalization.rs`
- Modify: `src-tauri/src/node_system/plan/model.rs`
- Test: `src-tauri/src/node_system/compiler/tests.rs`
- Test: `src-tauri/src/node_system/plan/validation.rs`

**Interfaces:**
- Rename `CachePolicy::None` to `CachePolicy::Disabled` without a serde alias.
- `PlannedOperation` gains stable operation identity, effective cache policy, execution-semantics version, workload class, and effective retry metadata used by later tasks.

```rust
pub struct OperationStableId(Box<str>);
pub struct ExecutionSemanticsVersion(u32);
pub struct AttemptId(u64);

pub enum WorkloadClass { Cpu, Io, AdapterIo, Exclusive }

pub struct PlannedRetry {
    pub idempotent: bool,
    pub policy: Option<RetryPolicy>,
}

pub struct PlannedOperation {
    pub stable_id: OperationStableId,
    pub source_node_id: NodeId,
    pub source_node_type_id: NodeTypeId,
    pub kernel: PlannedKernel,
    pub inputs: Box<[PlannedInput]>,
    pub outputs: Box<[PlannedOutput]>,
    pub params: CompiledParameterHandle,
    pub cache_policy: CachePolicy,
    pub semantics_version: ExecutionSemanticsVersion,
    pub workload: WorkloadClass,
    pub retry: PlannedRetry,
}
```

- [ ] **Step 1: Add compiler RED matrix**

Assert deterministic pure `PerRun` remains; nondeterministic/effectful `PerRun` becomes `Disabled`; plan specialization preserves values.

- [ ] **Step 2: Thread metadata through compiler layers**

Add fields consistently to `PendingOperation`, `IntermediateOperation`, and `PlannedOperation`. Effective policy is computed once in compilation.

- [ ] **Step 3: Remove legacy enum spelling**

Update Catalog declarations, fixtures, fingerprints, and tests. Do not retain `None` alias/default decoding.

- [ ] **Step 4: Run GREEN**

Run:

```text
pnpm rust:test --lib effective_cache_policy -- --test-threads=1
pnpm rust:test --lib node_system::plan -- --test-threads=1
pnpm rust:check
```

Expected: PASS.

---

### Task 11: Implement run-scoped per-run memoization and single-flight

**Files:**
- Create: `src-tauri/src/node_system/runtime/memoization.rs`
- Modify: `src-tauri/src/node_system/runtime/mod.rs`
- Modify: `src-tauri/src/node_system/runtime/run.rs`
- Modify: `src-tauri/src/node_system/runtime/scheduler.rs`
- Modify: `src-tauri/src/node_system/runtime/result_store.rs`
- Test: `src-tauri/src/node_system/runtime/tests.rs`

**Interfaces:**

```rust
pub struct RunMemoization;
pub struct ValueFingerprint([u8; 32]);
pub struct DemandFingerprint([u8; 32]);

pub struct OperationMemoKey {
    pub operation: OperationStableId,
    pub input_fingerprints: Box<[ValueFingerprint]>,
    pub resource_versions: ResourceVersionSet,
    pub semantics_version: ExecutionSemanticsVersion,
    pub demand: DemandFingerprint,
}

impl RunMemoization {
    pub fn get_or_produce(
        &self,
        key: OperationMemoKey,
        cancellation: &CancellationToken,
        produce: impl FnOnce() -> Result<Box<[RuntimeValue]>, RunError>,
    ) -> Result<Box<[RuntimeValue]>, RunError>;
}
```

- Cache owner is a single run; complete successful materialized values only.

- [ ] **Step 1: Add memoization RED matrix**

Test same key once, different inputs, different relevant revision, concurrent single-flight, producer error, cancellation, partial stream, finalization release, and new-run isolation.

- [ ] **Step 2: Implement canonical fingerprints**

Fingerprint typed scalar/artifact inputs deterministically. Streaming inputs are not cacheable until completely materialized by an explicit adapter.

- [ ] **Step 3: Implement single-flight state**

Waiters observe producer success/error without becoming producers. Cancellation of a waiter does not corrupt the producer entry; failed producer entries are removed.

- [ ] **Step 4: Integrate at operation execution**

Use only when `PlannedOperation.cache_policy == PerRun`. Keep the existing duplicate-activation guard separate.

- [ ] **Step 5: Run focused GREEN**

Run:

```text
pnpm rust:test --lib per_run_memoization -- --test-threads=1
pnpm rust:check
```

Expected: PASS with no leaked retained values.

---

### Task 12: Insert explicit materialization adapters at every contract boundary

**Files:**
- Modify: `src-tauri/src/node_system/compiler/relational.rs`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs`
- Modify: `src-tauri/src/node_system/plan/model.rs`
- Modify: `src-tauri/src/node_system/plan/validation.rs`
- Modify: `src-tauri/src/node_system/runtime/scheduler.rs`
- Modify: `src-tauri/src/node_system/runtime/relational.rs`
- Test: `src-tauri/src/node_system/compiler/tests.rs`
- Test: `src-tauri/src/node_system/runtime/tests.rs`

**Interfaces:**

```rust
pub enum PlannedAdapter {
    Collect { limits: MaterializationLimits },
    Buffer { capacity: usize },
    Spill { memory_limit_bytes: u64 },
    Replay,
    StreamBridge { format: StreamFormat },
}

pub enum PlannedKernel {
    Native(KernelHandle),
    Relational(RelationalSubplanIndex),
    Adapter(PlannedAdapter),
}
```

- Compiler selects adapters for native/native, native/relational, relational/native, and relational/relational edges.

- [ ] **Step 1: Add full contract-matrix RED**

Cover every `OutputProduction × InputConsumption` combination and all producer/consumer kinds. Assert deterministic plan shape independent of insertion/UUID order.

- [ ] **Step 2: Promote adapter selection**

Move the matrix from relational-only planning into a general compiler module and insert explicit adapter operations/value dependencies.

- [ ] **Step 3: Validate plan authority**

Reject incompatible direct edges and missing/extra adapters. Scheduler executes `PlannedAdapter`; it does not infer an adapter from runtime values.

- [ ] **Step 4: Run GREEN**

Run:

```text
pnpm rust:test --lib materialization_adapter -- --test-threads=1
pnpm rust:check
```

Expected: PASS.

---

### Task 13: Implement bounded materialization, spill, replay, and cleanup

**Files:**
- Create: `src-tauri/src/node_system/runtime/materialization.rs`
- Create: `src-tauri/src/node_system/runtime/spill.rs`
- Modify: `src-tauri/src/node_system/runtime/stream.rs`
- Modify: `src-tauri/src/node_system/runtime/run.rs`
- Modify: `src-tauri/src/node_system/runtime/relational.rs`
- Modify: `src-tauri/src/node_system/runtime/scheduler.rs`
- Test: `src-tauri/src/node_system/runtime/tests.rs`

**Interfaces:**

```rust
pub struct RunResourceBudgets {
    pub stream_capacity: NonZeroUsize,
    pub materialization_memory_bytes: u64,
    pub spill_directory_bytes: u64,
}

pub struct RunResourceOwner;
pub enum MaterializedArtifact {
    InMemory(Box<[Value]>),
    Spilled(SpillArtifact),
    Replayable(ReplayArtifact),
}
```

- Spill files and producer tasks are owned by `RunResourceOwner` and removed/drained on every terminal path.

- [ ] **Step 1: Add bounded-resource RED**

Test capacity-one backpressure, memory threshold spill, stable disk order, two replay passes, and cleanup after success/error/cancel/panic/deadline/project replacement.

- [ ] **Step 2: Replace `StreamValue::from_values` full buffering**

Use configured bounded capacity and an owned producer task rather than collecting all values and sizing the channel to the full input.

- [ ] **Step 3: Implement real spill/replay**

Serialize typed values to a run-private temporary file with bounded in-memory buffering. Replay owns independent cursors over immutable completed data.

- [ ] **Step 4: Integrate RAII cleanup and memoization rule**

Partial streams/spills never enter Task 11 cache. Cleanup errors are traced but do not overwrite the primary execution error.

- [ ] **Step 5: Run focused GREEN**

Run:

```text
pnpm rust:test --lib bounded_materialization -- --test-threads=1
pnpm rust:test --lib spill -- --test-threads=1
pnpm rust:check
```

Expected: PASS and temporary directories empty after each test.

---

### Task 14: Implement workload-aware bounded parallel scheduling

**Files:**
- Create: `src-tauri/src/node_system/runtime/scheduling.rs`
- Modify: `src-tauri/src/node_system/runtime/scheduler.rs`
- Modify: `src-tauri/src/node_system/runtime/run.rs`
- Modify: `src-tauri/src/node_system/plan/model.rs`
- Test: `src-tauri/src/node_system/runtime/tests.rs`

**Interfaces:**

```rust
pub struct SchedulingPolicy {
    pub cpu_parallelism: NonZeroUsize,
    pub io_parallelism: NonZeroUsize,
    pub adapter_parallelism: NonZeroUsize,
}

pub struct OperationCompletion {
    pub operation: OperationIndex,
    pub activation: ActivationId,
    pub attempt: AttemptId,
    pub outputs: Result<Box<[RuntimeValue]>, RunError>,
}
```

- Workers never mutate `Frame`; the scheduler commits completions on its owner thread.

- [ ] **Step 1: Add concurrency/fairness RED**

Test independent CPU overlap, hard class limits, separate I/O budget, effect/exclusive serialization, no I/O starvation under CPU load, deterministic value mapping despite completion order, and cancellation drain.

- [ ] **Step 2: Implement class queues and permits**

Admit ready operations with bounded weighted round-robin. Effect dependencies and exclusive resource requirements remain hard gates.

- [ ] **Step 3: Add completion ownership**

Workers receive immutable operation inputs/context and return `OperationCompletion`; only the scheduler mutates frames, publishes outputs, and marks dependencies complete.

- [ ] **Step 4: Run GREEN**

Run:

```text
pnpm rust:test --lib parallel_scheduler -- --test-threads=1
pnpm rust:test --lib effect_dependencies_determine_ready_queue_order -- --test-threads=1
pnpm rust:check
```

Expected: PASS without timing-only sleeps; tests use barriers/condition signaling.

---

### Task 15: Add one propagated deadline and typed timeout outcomes

**Files:**
- Modify: `src-tauri/src/node_system/runtime/run.rs`
- Modify: `src-tauri/src/node_system/runtime/stream.rs`
- Modify: `src-tauri/src/node_system/runtime/scheduler.rs`
- Modify: `src-tauri/src/node_system/runtime/materialization.rs`
- Modify: `src-tauri/src/node_system/runtime/execution_event.rs`
- Modify: `src-tauri/src/commands/node_system_execution_dto.rs`
- Modify: `src/shared/types/dto/runEvent.ts`
- Modify: `src/shared/types/dto/runEventParser.ts`
- Test: `src-tauri/src/node_system/runtime/tests.rs`
- Test: `src/shared/types/dto/runEventParser.test.ts`
- Test: `src/services/nodeSystem/nodeSystemGoldenContracts.test.ts`

**Interfaces:**

```rust
pub struct RunOptions {
    pub deadline: Option<RunDeadline>,
    pub budgets: RunResourceBudgets,
    pub scheduling: SchedulingPolicy,
}

pub enum RunError {
    DeadlineExceeded { phase: RunPhase },
    // existing variants
}
```

- Cancellation wins when cancellation and deadline are simultaneously observable.

- [ ] **Step 1: Add phase-specific timeout RED**

Cover queue wait, kernel, bounded send/recv, adapter I/O, publication, and cleanup. Assert no late result and all resources released.

- [ ] **Step 2: Implement deadline-aware waits**

Use monotonic `Instant` and condition-variable timeout waits. Do not create independent timers in each adapter.

- [ ] **Step 3: Extend strict wire**

Add stable deadline error code/phase to Rust and TS golden contracts and strict parser.

- [ ] **Step 4: Run GREEN**

Run:

```text
pnpm rust:test --lib deadline -- --test-threads=1
pnpm test src/shared/types/dto/runEventParser.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts
```

Expected: PASS.

---

### Task 16: Implement safe opt-in retry for idempotent operations

**Files:**
- Modify: `src-tauri/src/node_system/protocol/model.rs`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs`
- Modify: `src-tauri/src/node_system/plan/model.rs`
- Modify: `src-tauri/src/node_system/runtime/scheduler.rs`
- Modify: `src-tauri/src/node_system/runtime/run.rs`
- Test: `src-tauri/src/node_system/compiler/tests.rs`
- Test: `src-tauri/src/node_system/runtime/tests.rs`

**Interfaces:**

```rust
pub struct RetryPolicy {
    pub max_attempts: NonZeroU32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
}

pub enum KernelErrorKind { Permanent, Transient }

pub struct PlannedRetry {
    pub idempotent: bool,
    pub policy: Option<RetryPolicy>,
}
```

- Side-effecting database/variable/filesystem/effect/call operations compile with no retry regardless of untrusted request input.
- Stable operation identity persists; each attempt gets distinct `AttemptId` and `ActivationId`.

- [ ] **Step 1: Add compiler safety RED**

Assert pure idempotent protocols retain retry; nondeterministic/effectful/write protocols are forced to no retry.

- [ ] **Step 2: Add runtime RED matrix**

Cover transient success after retry, permanent no-retry, max attempts, insufficient deadline, cancellation during backoff, distinct attempt identity, and failed/partial attempts not cached.

- [ ] **Step 3: Implement typed attempt loop**

Backoff is bounded, cancellation/deadline aware, and never sleeps while holding project/frame/cache locks.

- [ ] **Step 4: Run GREEN**

Run:

```text
pnpm rust:test --lib retry -- --test-threads=1
pnpm rust:check
```

Expected: PASS.

---

### Task 17: Restore complete Catalog search with deterministic pinyin

**Files:**
- Modify: `package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `src-tauri/src/node_system/catalog/localization.rs`
- Modify: `src-tauri/src/project/project_reads.rs`
- Modify: `src/shared/types/dto/localizedCatalog.ts`
- Create: `src/features/domain/nodeCatalog/searchDocument.ts`
- Modify: `src/features/domain/nodeCatalog/search.ts`
- Modify: `src/features/core/nodeCatalog/localizedSearchIndex.ts`
- Test: `src-tauri/src/node_system/catalog/tests.rs`
- Test: `src/services/nodeSystem/catalogService.test.ts`
- Test: `src/features/domain/nodeCatalog/search.test.ts`
- Test: `src/features/core/nodeCatalog/localizedSearchIndex.test.ts`
- Test: `src/views/EditorView/Layout/NodePalette.test.tsx`

**Interfaces:**

```ts
export interface CatalogSearchDocument {
  nodeTypeId: string
  localizedTitle: string
  aliases: string[]
  technicalTerms: string[]
  backendSearchText: string[]
  resourceNames: string[]
  pinyinFull: string[]
  pinyinInitials: string[]
}

export function buildCatalogSearchDocument(
  item: LocalizedCatalogItemDto,
): CatalogSearchDocument
```

- Rust DTO uses distinct `backend_search_text` and `resource_names` arrays; old singular `pinyin`/`search_text` fields are rejected.
- Pinyin is generated offline with `pinyin-pro@3.28.2` behind the domain function.

- [ ] **Step 1: Install the pinned dependency**

Run: `pnpm add pinyin-pro@3.28.2`

Expected: `package.json` and `pnpm-lock.yaml` update only dependency metadata.

- [ ] **Step 2: Write wire/search RED**

Replace tests that explicitly exclude metadata. Cover title, aliases, technical terms, stable node type ID, backend text, resource name, full pinyin, initials, mixed Chinese/Latin, polyphonic fixture, unknown characters, and locale identity stability.

- [ ] **Step 3: Expand Rust Catalog metadata**

Populate distinct arrays from static and resource entries. Do not flatten fields into a single opaque string.

- [ ] **Step 4: Build one frontend search document**

Normalize every field and generate full/initial pinyin in `searchDocument.ts`; both index and direct search consume it. Results preserve original item/stable ID.

- [ ] **Step 5: Run focused GREEN**

Run:

```text
pnpm test src/services/nodeSystem/catalogService.test.ts src/features/domain/nodeCatalog/search.test.ts src/features/core/nodeCatalog/localizedSearchIndex.test.ts src/views/EditorView/Layout/NodePalette.test.tsx
pnpm rust:test --lib node_system::catalog::tests -- --test-threads=1
pnpm typecheck
```

Expected: PASS.

---

### Task 18: Replace status events with paired hierarchical timed trace spans

**Files:**
- Modify: `src-tauri/src/node_system/analysis/observability.rs`
- Modify: `src-tauri/src/node_system/analysis/trace_store.rs`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs`
- Modify: `src-tauri/src/node_system/runtime/scheduler.rs`
- Modify: `src-tauri/src/commands/command_trace.rs`
- Modify: `src/shared/types/dto/trace.ts`
- Modify: `src/services/nodeSystem/traceService.ts`
- Modify: `src/features/application/observability/useGraphTraceDetails.ts`
- Test: `src-tauri/src/commands/command_trace_tests.rs`
- Test: `src-tauri/src/project/project_trace_query_tests.rs`
- Test: `src-tauri/src/node_system/runtime/tests.rs`
- Test: `src/services/nodeSystem/traceService.test.ts`
- Test: `src/features/application/observability/useGraphTraceDetails.test.tsx`

**Interfaces:**

```rust
pub struct TraceSpan {
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,
    pub run_id: Option<RunId>,
    pub operation_id: Option<OperationStableId>,
    pub activation_id: Option<ActivationId>,
    pub attempt_id: Option<AttemptId>,
    pub kind: SpanKind,
    pub started_at: MonotonicTimestamp,
    pub finished_at: MonotonicTimestamp,
    pub outcome: SpanOutcome,
    pub correlation: CorrelationContext,
}
```

- `TraceSink::start_span(SpanSpec) -> SpanGuard`; normal paths call `finish`, Drop supplies only an internal-aborted fallback.

- [ ] **Step 1: Add span-model RED**

Assert unique IDs, exact parent pairing, nonnegative monotonic duration, run/operation/activation/attempt identity, and success/error/cancel/timeout/retry/cleanup outcomes.

- [ ] **Step 2: Add strict Rust↔TS golden RED**

Use decimal strings for IDs/timestamps beyond JS safe integer range. Reject missing finish/outcome/parent fields and old status-event shapes.

- [ ] **Step 3: Implement span guard and clock abstraction**

Compiler spans cover snapshot/analysis/lowering. Runtime spans cover run, operation attempt, resource acquire, adapter I/O, publication, and cleanup.

- [ ] **Step 4: Update stores, queries, and frontend projection**

Trace records are self-contained completed spans. UI computes duration from DTO fields and does not pair status events heuristically.

- [ ] **Step 5: Run focused GREEN**

Run:

```text
pnpm rust:test --lib trace_span -- --test-threads=1
pnpm test src/services/nodeSystem/traceService.test.ts src/features/application/observability/useGraphTraceDetails.test.tsx
pnpm typecheck
```

Expected: PASS.

---

### Task 19: Remove extra module and legacy compatibility boundaries, then close TODO

**Files:**
- Move: `src-tauri/src/node_system/parameter_types/dataframe.rs` → `src-tauri/src/node_system/protocol/dataframe.rs`
- Delete: `src-tauri/src/node_system/parameter_types/mod.rs`
- Modify: `src-tauri/src/node_system/protocol/mod.rs`
- Modify: `src-tauri/src/node_system/mod.rs`
- Modify: `src-tauri/src/node_system/analysis/projection.rs`
- Modify: `src-tauri/src/node_system/analysis/mod.rs`
- Modify: `src-tauri/src/node_system/catalog/localization.rs`
- Modify: `src-tauri/src/node_system/document/history.rs`
- Modify: `src-tauri/src/node_system/document/tests.rs`
- Modify: `src-tauri/src/node_system/testing/source_audit.rs`
- Modify: `src-tauri/src/node_system/analysis/projection.rs`
- Modify: `src-tauri/src/node_system/catalog/builtin.rs`
- Modify: `src-tauri/src/node_system/catalog/dataframe/mod.rs`
- Modify: `src-tauri/src/node_system/catalog/tests.rs`
- Modify: `src-tauri/src/node_system/compiler/schema_analysis.rs`
- Modify: `src-tauri/src/node_system/compiler/task1_tests.rs`
- Modify: `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs`
- Modify: `src-tauri/src/node_system/registry/validation.rs`
- Modify: `src-tauri/src/node_system/registry/tests.rs`
- Modify: `TODO.md:293-316`
- Modify: `docs/superpowers/specs/2026-08-07-node-architecture-todo-closure-design.md`
- Modify: `docs/superpowers/plans/2026-08-07-node-architecture-todo-closure.md`

**Interfaces:**
- DataFrame protocol types live only under `crate::node_system::protocol::dataframe`.
- `LocalizationLookup` is the only localization trait; `BuiltinLocalizationBundle` implements it directly.
- `ProjectHistoryTransaction.persistence` is required on wire; missing persistence fails deserialization.

- [ ] **Step 1: Add architecture/strict-wire RED**

Audit rejects the old directory/module/import/re-export, `LocalizationBundle`, `Compatibility boundary`, blanket bridge, and persistence default. Replace `legacy_history_transaction_defaults_to_in_memory_until_save` with `history_transaction_rejects_missing_persistence`.

- [ ] **Step 2: Move protocol ownership without shim**

Move the file, update direct imports in analysis/catalog/compiler/registry/document tests, and delete the old module declaration/directory. Do not re-export old paths.

- [ ] **Step 3: Merge localization interfaces**

Delete `LocalizationBundle`; implement `LocalizationLookup` directly and update exports/tests.

- [ ] **Step 4: Make History persistence strict**

Remove `#[serde(default)]` from `persistence`; retain optional defaults only for genuinely optional snapshot/graph-move members. Verify all three valid policies round-trip.

- [ ] **Step 5: Run focused boundary GREEN**

Run:

```text
pnpm rust:test --lib source_audit -- --test-threads=1
pnpm rust:test --lib history_transaction_rejects_missing_persistence -- --test-threads=1
pnpm rust:test --lib node_system::protocol::dataframe -- --test-threads=1
pnpm rust:check
```

Expected: PASS and no old path/symbol remains.

- [ ] **Step 6: Run complete verification**

Run in order:

```text
pnpm typecheck
pnpm test
pnpm rust:fmt:check
pnpm rust:check
pnpm rust:test --lib -- --test-threads=1
pnpm verify
git --no-pager diff --check
git --no-pager diff --cached --name-only
git --no-optional-locks status --short
```

Expected:

- frontend and Rust suites report zero failures;
- canonical `pnpm verify` exits 0 with no `LNK1102`;
- `git diff --check` emits no output;
- `git diff --cached --name-only` emits no output;
- status contains only intended unstaged/untracked source, test, spec, plan, report, and TODO changes.

- [ ] **Step 7: Perform final scope and quality reviews**

Review every `TODO.md:293-316` requirement against implementation and tests. Fix all Critical/Important findings with focused RED-GREEN rounds. Only after review is READY with 0 Critical / 0 Important, check every completed TODO and replace the historical `LNK1102` exception text with the fresh passing evidence while preserving historical runs as historical.
