# Project Publication Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the arrival-ordered frontend resource-result applier with one project-scoped, revision-ordered publication coordinator that prepares moves without store mutation, recovers every affected projection from one coherent backend baseline, and deterministically settles all callers.

**Architecture:** Rust extends the already-authoritative `ProjectIndex` response with history captured under the same publication boundary as `projectInstanceId` and `publicationRevision`; the existing move wire contract remains singular and requires the authoritative destination `name`. React delegates every direct result and `ResourceMutationCommitted` event to one `ProjectPublicationCoordinator`, which owns a revision-keyed queue, performs all asynchronous preparation before a synchronous non-throwing commit, and replaces gaps with a full authoritative reconciliation rather than replay. A final integration task removes alternate publication ownership, preserves and repairs the mismatched Task 2 report artifacts, and requires a fresh whole-Task-2 review before the filesystem plan may resume Task 3.

**Tech Stack:** Rust 2024, serde/serde_json, Tauri 2, TypeScript 5.8, React 19, Zustand, Vitest 4, pnpm.

## Global Constraints

- `ProjectState.project_data` remains the authoritative project, graph, function, variable, and worksheet state; frontend stores are projections only.
- `ProjectState::insert_graph` remains the only backend graph insertion path.
- `ProjectPublicationCoordinator` is the sole frontend owner of resource-result acceptance, publication ordering, move application, history publication, watermark publication, gap recovery, and duplicate waiter settlement.
- A publication is normally eligible only when `publicationRevision === appliedRevision + 1`; arrival order never authorizes installation.
- A second payload for the same project instance and publication revision must share the existing promise only when its canonical fingerprint matches; a different fingerprint fails with `publication_protocol_error` and has zero store effects.
- Every fallible asynchronous operation—including destination preload, loaded/invalidated/caller graph hydration, function metadata refresh, and recovery snapshot fetch—finishes before any path-owned store mutation.
- A `false` preload or hydration result is failure, never success.
- Move and recovery commits revalidate project identity plus epoch once more, then mutate stores synchronously with no intervening `await`; prevalidated commit functions are non-throwing.
- History status and the publication watermark are published last, after the complete store commit.
- Recovery uses one `ProjectIndex` response containing project identity, resource publication revision, authoritative graph paths/display names, function signatures/revisions, variables/worksheets, and backend history status.
- Recovery hydrates every graph loaded when recovery starts, every expected/invalidated graph path in queued publications, and every queued move destination.
- Recovery never automatically replays a mutation at or below the recovered backend revision; those waiters resolve `{ status: 'recovered' }`.
- Recovery failure rejects every waiter owned by that attempt with `publication_recovery_failed`, clears its pending/in-flight state, leaves history/watermark unchanged, marks the current project projection stale, and permits a later submission to start a fresh attempt.
- Project replacement increments the frontend epoch, rejects all old pending/recovering waiters with `stale_project_lifecycle`, clears queue/recovery state, and establishes the replacement baseline from its index.
- Every asynchronous continuation checks its captured project instance and epoch before any side effect.
- `ResourceMoveDto` remains the only move wire shape and requires `{ from, to, kind, name }`; rename, undo, and redo supply the authoritative destination name.
- Do not add a compatibility event, alternate result applier, dual watermark owner, automatic mutation replay, retry loop, browser dialog, or second UI library.
- Preserve unrelated working-tree changes. Do not reset, overwrite, stage, commit, or otherwise rewrite work outside the files named by the active task.
- Do not create commits in any task.
- Strict TDD applies: write the named regression first, run the exact RED command and observe the stated failure, make the minimum production change, then run the exact GREEN commands.
- Frontend tests use only the explicit file-by-file `pnpm exec vitest run` commands listed below; do not run unqualified `pnpm test`, `pnpm verify:frontend`, or `pnpm verify`.
- Rust tests use only the listed focused `--lib` filters with `CARGO_BUILD_JOBS=1` and `--test-threads=1`; do not run unfiltered `cargo test`, unfiltered `pnpm rust:test`, `pnpm rust:test:sci`, `pnpm verify:rust`, or `pnpm verify`.
- This sub-plan does not resume Task 3 of `docs/superpowers/plans/2026-07-27-project-filesystem-transaction.md`; only a clean Task 2 re-review recorded in Task 3 below may reopen that gate.

## Planned File Structure

- `src-tauri/src/project/project_io.rs`: extend `ProjectIndex` with the backend history baseline already protected by the publication boundary.
- `src-tauri/src/project/project_state.rs`: stamp identity, publication revision, function metadata, variables, and history into one owned index overlay.
- `src-tauri/src/commands/command_project/query.rs`: keep `get_project_index` thin and test the coherent recovery wire baseline.
- `src/services/project/projectService.ts`: type the required `ProjectIndexRow.history` field.
- `src/features/application/editorMutation/projectPublicationCoordinator.ts`: own project epoch, ordered queue, fingerprints, draining, recovery, settlement, and the production singleton.
- `src/features/application/editorMutation/projectPublicationMovePlan.ts`: prepare immutable graph-move plans and commit them synchronously without I/O.
- `src/features/application/editorMutation/projectPublicationRecovery.ts`: collect recovery graph membership, prepare a complete authoritative reconciliation, commit it synchronously, and mark failure stale.
- `src/features/application/editorMutation/resourceMutationResult.ts`: retain only pure wire/delta validation and pure synchronous publication-commit helpers; remove queue, recovery, lifecycle, history, and watermark ownership.
- `src/features/application/editorMutation/projectPublicationCoordinator.test.ts`: focused ordering, duplicate, prepare/commit, recovery, retry, and lifecycle regressions.
- `src/features/application/editorMutation/projectPublicationIntegration.test.ts`: prove all event/direct mutation paths delegate to the singleton and no alternate owner remains.
- `.superpowers/sdd/2026-07-25-revisioned-mutation-history/task-2-report.pre-publication-recovery-repair.md`: immutable copy of the pre-repair 2026-07-25 Task 2 report.
- `.superpowers/sdd/2026-07-27-project-filesystem-transaction/task-2-report.pre-publication-recovery-repair.md`: immutable copy of the pre-repair 2026-07-27 Task 2 report.
- `.superpowers/sdd/2026-07-27-project-filesystem-transaction/project-publication-recovery-report.md`: fresh RED/GREEN and scope evidence for this sub-plan.
- `.superpowers/sdd/2026-07-27-project-filesystem-transaction/project-publication-recovery-review-package.md`: focused diff/test package for the mandatory re-review.
- `.superpowers/sdd/2026-07-25-revisioned-mutation-history/task-2-report.md`: append an evidence-provenance repair note without deleting historical content.
- `.superpowers/sdd/2026-07-27-project-filesystem-transaction/task-2-report.md`: append the supersession/recovery result and corrected artifact provenance without deleting the five-round history.
- `.superpowers/sdd/2026-07-27-project-filesystem-transaction/progress.md`: retain the blocked ledger entry and append the recovery result plus the explicit Task 2 re-review gate.

---

### Task 1: Add one coherent backend publication-recovery baseline

**Files:**
- Modify: `src-tauri/src/project/project_io.rs:106-120, 358-369`
- Modify: `src-tauri/src/project/project_state.rs:665-709`
- Modify: `src-tauri/src/commands/command_project/query.rs:54-109, 149-426`
- Modify: `src/services/project/projectService.ts:37-74`
- Test: `src-tauri/src/commands/command_project/query.rs`

**Interfaces:**
- Consumes: existing `ProjectIndex`, `HistoryStatusDto`, `ProjectState::overlay_project_index_if_owned`, `MutationPublication.resource_revision`, authoritative `ProjectData`, and the existing required `ResourceMoveDto.name` wire field.
- Produces:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIndex {
    pub project_instance_id: String,
    #[serde(default)]
    pub publication_revision: u64,
    #[serde(default)]
    pub history: crate::node_system::document::HistoryStatusDto,
    pub project_name: String,
    pub app_version: String,
    pub export_time: String,
    pub graphs: Vec<ProjectGraphIndexEntry>,
    #[serde(default)]
    pub worksheets: Vec<ProjectWorksheetIndexEntry>,
    #[serde(default)]
    pub variables: Vec<ProjectVariableIndexEntry>,
}
```

```ts
export interface ProjectIndexRow {
  projectInstanceId: string;
  projectName: string;
  appVersion: string;
  exportTime: string;
  publicationRevision: number;
  history: HistoryStatusDto;
  graphs: ProjectGraphIndexRow[];
  worksheets?: ProjectWorksheetIndexRow[];
  variables?: ProjectVariableIndexRow[];
}
```

- `ProjectState::overlay_project_index_if_owned(...)` sets `projectInstanceId`, `publicationRevision`, authoritative globals/functions, and `history` while holding the existing `mutation_publication` boundary; it does not perform filesystem I/O or emit events.
- No new Tauri command is introduced: `ProjectService.getProjectIndex()` is the one canonical recovery-snapshot request.

- [ ] **Step 1: Add the failing coherent-baseline Rust regression**

Add this exact test to `commands::command_project::query::tests`:

```rust
#[test]
fn project_index_carries_one_coherent_publication_recovery_baseline() {
    let root = std::env::temp_dir().join(format!(
        "yssbi-publication-recovery-index-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    crate::project::save_project_to_file(
        &ProjectData::new(),
        root.to_string_lossy().as_ref(),
    )
    .unwrap();

    let state = ProjectState::new();
    state.activate_loaded_project(root.to_string_lossy().into_owned(), ProjectData::new());
    let graph_path = crate::project::GraphResourcePath::new(
        "events/Recovery.yssbi-event",
    )
    .unwrap();
    state
        .insert_graph(
            graph_path.clone(),
            crate::project::GraphResourceDocument::new(
                "Recovery",
                crate::project::GraphDocumentKind::Event,
            ),
        )
        .unwrap();
    state
        .apply_editor_graph_mutation(
            &graph_path,
            "en-US",
            editor_create_node_request(&graph_path),
        )
        .unwrap();

    let index = get_project_index_with_reader(&state, |_| Ok(index_named("Recovery"))).unwrap();

    assert_eq!(index.project_instance_id, state.project_instance_id());
    assert_eq!(index.publication_revision, 1);
    assert_eq!(index.history, state.history_status());
    assert!(index.history.can_undo);
    assert!(!index.history.can_redo);
    std::fs::remove_dir_all(root).unwrap();
}
```

Add the local `editor_create_node_request(&GraphResourcePath) -> MutationRequest<EditorGraphMutationDto>` fixture in the same test module using one `yssbi.constant.int64` node at `{ x: 10.0, y: 20.0 }`, `GraphRevision::INITIAL`, `OperationId::new()`, empty parameters, and no user label. Do not reach through a production-only helper or alter the mutation API for this fixture.

- [ ] **Step 2: Run the exact Rust test and verify RED**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- commands::command_project::query::tests::project_index_carries_one_coherent_publication_recovery_baseline --exact --test-threads=1
```

Expected: compilation fails because `ProjectIndex` has no `history` field, or the assertion cannot compile against that missing recovery baseline. Do not proceed if the test passes before production changes.

- [ ] **Step 3: Add history to the index wire type and every explicit constructor**

Add `history` exactly as shown in the produced interface. Set `history: Default::default()` in disk-only `read_project_index` construction and every test fixture that constructs `ProjectIndex` directly; disk history is never authoritative and is overwritten by the owned state overlay before IPC return.

In `src/services/project/projectService.ts`, import `HistoryStatusDto` from `@/shared/types/dto/editorMutation` and make `ProjectIndexRow.history` required. Update explicit frontend index fixtures in the Task 2/3 test files named by subsequent steps to include `{ canUndo: false, canRedo: false }`; do not make the field optional or synthesize it in the coordinator.

- [ ] **Step 4: Stamp history under the existing publication boundary**

Inside `ProjectState::overlay_project_index_if_owned`, preserve the lock order `mutation_publication -> project_path -> project_data -> history`. After identity/path revalidation and authoritative variable/function overlay, assign:

```rust
index.project_instance_id = publication.project_instance_id.clone();
index.publication_revision = publication.resource_revision;
index.history = self.history.read().unwrap().status();
```

Do not call `ProjectState::history_status()` while already holding `mutation_publication`, because that method reacquires the same mutex. Keep filesystem index reading outside all four locks as it is now.

- [ ] **Step 5: Run Task 1 GREEN checks**

Run sequentially:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- commands::command_project::query::tests::project_index_carries_one_coherent_publication_recovery_baseline --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- commands::command_project::query::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- event::event_project::tests::resource_mutation_result_serializes_explicit_graph_move_identity --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
pnpm exec vitest run src/features/application/editorMutation/editorMutation.test.ts src/features/core/dataStore/projectIOStore.test.ts
git diff --check
```

Expected: the coherent-baseline test, query module, existing authoritative move-name serde test, and the two explicit frontend contract files pass; Rust check/format and whitespace checks pass. Do not run any broader Rust or frontend suite.

---

### Task 2: Implement the project-scoped revision queue, two-phase moves, recovery, and lifecycle cancellation

**Files:**
- Create: `src/features/application/editorMutation/projectPublicationCoordinator.ts`
- Create: `src/features/application/editorMutation/projectPublicationMovePlan.ts`
- Create: `src/features/application/editorMutation/projectPublicationRecovery.ts`
- Create: `src/features/application/editorMutation/projectPublicationCoordinator.test.ts`
- Modify: `src/features/application/editorMutation/resourceMutationResult.ts:1-634`
- Delete: `src/features/application/editorMutation/resourceMutationResult.test.ts`
- Delete: `src/features/application/editor/migrateGraphResourcePath.ts`
- Delete: `src/features/application/editor/migrateGraphResourcePath.test.ts`
- Create: `src/features/application/editorMutation/projectPublicationMovePlan.test.ts`
- Modify: `src/features/application/editorProjection/graphProjectionCoordinator.ts:108-228`
- Modify: `src/features/core/dataStore/projectIOStore.ts:112-178, 201-340`
- Modify: `src/features/core/dataStore/projectClientReset.ts:5-38`
- Modify: `src/features/application/graphDocument/functionSignatureSync.ts:66-93`
- Modify: `src/features/core/sync/handlers/ProjectMutationEventHandler.ts:1-89`
- Modify: `src/features/application/editorMutation/functionSignatureCoordinator.ts`
- Modify: `src/features/application/editorMutation/historyCoordinator.ts`
- Modify: `src/features/application/resource/resourceActions.ts`
- Modify: `src/features/application/editor/closeEditorTab.ts`
- Modify: `src/features/application/editor/useWorksheetManagement.ts`
- Test: `src/features/application/editorMutation/projectPublicationCoordinator.test.ts`
- Test: `src/features/application/editorMutation/projectPublicationMovePlan.test.ts`

**Interfaces:**
- Consumes: Task 1 `ProjectIndexRow.history`, existing `ResourceMutationResultDto`, pure wire/delta validators, `GraphProjectionService`, Zustand `getState`/`setState`, `reconcileResourceSnapshot`, `applySnapshotDocumentPatches`, `reconcileOpenLayoutTabsWithResources`, and current graph/resource/session/tab/document/variable/history stores.
- Produces:

```ts
export type ProjectPublicationSuccess =
  | { status: 'applied'; affectedGraphPaths: ReadonlySet<string> }
  | { status: 'duplicate'; affectedGraphPaths: ReadonlySet<string> }
  | { status: 'recovered'; affectedGraphPaths: ReadonlySet<string> };

export type ProjectPublicationErrorCode =
  | 'stale_project_lifecycle'
  | 'publication_protocol_error'
  | 'publication_recovery_failed';

export class ProjectPublicationError extends Error {
  constructor(
    readonly code: ProjectPublicationErrorCode,
    message: string,
    options?: { cause?: unknown },
  );
}

export interface ProjectPublicationSubmission {
  result: ResourceMutationResultDto;
  fallbackPaths?: readonly string[];
  validate?: (result: ResourceMutationResultDto) => string | undefined;
}

export interface ProjectPublicationDependencies {
  loadRecoverySnapshot(): Promise<ProjectIndexRow>;
  prepareGraphProjection(
    graphPath: string,
    projectInstanceId: string,
    epoch: number,
  ): Promise<EditorGraphProjectionDto | false>;
  captureLoadedGraphPaths(): ReadonlySet<string>;
  prepareMove(
    move: ResourceMoveDto,
    preparedDestination: EditorGraphProjectionDto,
  ): PreparedGraphResourceMove;
  commitPublication(plan: PreparedProjectPublication): void;
  commitRecovery(plan: PreparedProjectRecovery): void;
  markProjectProjectionStale(): void;
}

export class ProjectPublicationCoordinator {
  constructor(dependencies: ProjectPublicationDependencies);
  startProject(projectInstanceId: string, appliedRevision: number): void;
  cancelProject(): void;
  submit(input: ProjectPublicationSubmission): Promise<ProjectPublicationSuccess>;
  getSnapshotForTests(): {
    projectInstanceId: string | null;
    epoch: number;
    appliedRevision: number;
    phase: 'idle' | 'applying' | 'recovering';
    pendingRevisions: number[];
  };
}

export const projectPublicationCoordinator: ProjectPublicationCoordinator;
```

```ts
export interface PreparedGraphResourceMove {
  readonly from: string;
  readonly to: string;
  readonly kind: 'event' | 'function';
  readonly name: string;
  readonly destinationProjection: EditorGraphProjectionDto;
  readonly resourceSnapshot: PreparedResourceMoveSnapshot;
  readonly documentSnapshot: PreparedDocumentMoveSnapshot;
  readonly tabSnapshot: PreparedTabMoveSnapshot;
  readonly sessionSnapshot: PreparedSessionMoveSnapshot;
  readonly referenceSnapshot: PreparedGraphReferenceMoveSnapshot;
  readonly variableScopeSnapshot: PreparedVariableScopeMoveSnapshot;
}

export function prepareGraphResourceMove(
  move: ResourceMoveDto,
  destinationProjection: EditorGraphProjectionDto,
): PreparedGraphResourceMove;

export function commitGraphResourceMove(plan: PreparedGraphResourceMove): void;
```

```ts
export interface PreparedProjectPublication {
  readonly projectInstanceId: string;
  readonly epoch: number;
  readonly publicationRevision: number;
  readonly fingerprint: string;
  readonly affectedGraphPaths: ReadonlySet<string>;
  readonly moves: readonly PreparedGraphResourceMove[];
  readonly projectionReplacements: readonly GraphProjectionReplacementDto[];
  readonly functionInstalls: readonly PreparedFunctionDeltaInstall[];
  readonly variableInstalls: readonly PreparedVariableDeltaInstall[];
  readonly history: HistoryStatusDto;
}

export interface PreparedProjectRecovery {
  readonly projectInstanceId: string;
  readonly epoch: number;
  readonly publicationRevision: number;
  readonly index: ProjectIndexRow;
  readonly projections: ReadonlyMap<string, EditorGraphProjectionDto>;
  readonly graphPathsLoadedAtStart: ReadonlySet<string>;
}
```

- `resourceMutationResult.ts` exports only `collectResourceMutationGraphPaths`, `validateResourceMutationWireResult`, `fingerprintResourceMutationResult`, `prepareSynchronousPublicationCommit`, and `commitPreparedPublication`; it contains no module-level project identity, epoch, queue, drain, recovery, promise registry, watermark, or history state.
- `migrateGraphResourcePath.ts` becomes the pure prepare/synchronous commit owner shown above. Its old async `migrateGraphResourcePath(...)` export is removed, not wrapped.
- Direct callers and the event handler call `projectPublicationCoordinator.submit(...)` and do not install history, mutate move-owned stores, update a publication revision, or trigger independent recovery.

- [ ] **Step 1: Add the focused coordinator regressions before creating the coordinator**

Create `projectPublicationCoordinator.test.ts` with deterministic injected dependencies and these exact test names:

```text
ProjectPublicationCoordinator > queues reverse arrival N+1 then N without installing N+1 first
ProjectPublicationCoordinator > rechecks the watermark when missing N arrives during recovery I/O
ProjectPublicationCoordinator > rejects every recovery-owned waiter and clears pending state when snapshot fetch fails
ProjectPublicationCoordinator > permits a later submission to start a fresh recovery after failure
ProjectPublicationCoordinator > shares one promise and one commit for matching direct and event deliveries
ProjectPublicationCoordinator > rejects a different fingerprint at the same revision with publication_protocol_error
ProjectPublicationCoordinator > performs no path-owned mutation when destination preload returns false
ProjectPublicationCoordinator > performs no path-owned mutation when caller hydration returns false
ProjectPublicationCoordinator > retries a failed move without losing metadata or document flags
ProjectPublicationCoordinator > installs authoritative destination names for rename undo and redo
ProjectPublicationCoordinator > recovers resources functions projections history and watermark from one snapshot
ProjectPublicationCoordinator > settles revisions at or below the recovered watermark without replay
ProjectPublicationCoordinator > rejects queued and recovering work when the project lifecycle changes
```

Use this state recorder in the test file so prepare and commit effects are distinguishable:

```ts
interface RecordedProjectionState {
  resources: string[];
  names: Record<string, string>;
  documentFlags: Record<string, { dirty: boolean; stale: boolean; conflict: boolean }>;
  projections: string[];
  functionRevisions: Record<string, number>;
  history: { canUndo: boolean; canRedo: boolean };
  watermark: number;
  commitOrder: number[];
}

function createHarness() {
  const state: RecordedProjectionState = {
    resources: ['events/Before.yssbi-event'],
    names: { 'events/Before.yssbi-event': 'Before' },
    documentFlags: {
      'events/Before.yssbi-event': { dirty: true, stale: false, conflict: false },
    },
    projections: ['events/Before.yssbi-event'],
    functionRevisions: {},
    history: { canUndo: false, canRedo: false },
    watermark: 0,
    commitOrder: [],
  };
  const snapshotRequests: Array<ReturnType<typeof deferred<ProjectIndexRow>>> = [];
  const projectionRequests = new Map<string, ReturnType<typeof deferred<EditorGraphProjectionDto | false>>>();
  const dependencies: ProjectPublicationDependencies = {
    loadRecoverySnapshot: vi.fn(() => {
      const request = deferred<ProjectIndexRow>();
      snapshotRequests.push(request);
      return request.promise;
    }),
    prepareGraphProjection: vi.fn((path) => {
      const request = projectionRequests.get(path);
      if (!request) throw new Error(`missing projection request for ${path}`);
      return request.promise;
    }),
    captureLoadedGraphPaths: vi.fn(() => new Set(state.projections)),
    prepareMove: vi.fn((move, projection) => prepareRecordedMove(state, move, projection)),
    commitPublication: vi.fn((plan) => commitRecordedPublication(state, plan)),
    commitRecovery: vi.fn((plan) => commitRecordedRecovery(state, plan)),
    markProjectProjectionStale: vi.fn(),
  };
  return { state, dependencies, snapshotRequests, projectionRequests };
}
```

Each test snapshots `state` before resolving preparation and asserts exact equality until commit. The recovery-race test submits revision 2, waits for `loadRecoverySnapshot`, submits and resolves revision 1, then resolves a snapshot at revision 2; it must resolve revision 1 as `applied`, revision 2 as `recovered` or `duplicate` according to whether revision 2 was committed before snapshot reconciliation, and must never reject either as a gap. The failed-recovery tests assert `pendingRevisions: []` and `phase: 'idle'` after rejection.

- [ ] **Step 2: Run the exact coordinator file and verify RED**

Run:

```sh
pnpm exec vitest run src/features/application/editorMutation/projectPublicationCoordinator.test.ts
```

Expected: FAIL because `projectPublicationCoordinator.ts`, `projectPublicationMovePlan.ts`, and their interfaces do not exist. Do not create stubs that make the tests pass without the required behavior.

- [ ] **Step 3: Implement queue state, canonical fingerprints, and deterministic settlement**

Implement one private state object in `ProjectPublicationCoordinator`:

```ts
interface ProjectPublicationState {
  projectInstanceId: string | null;
  epoch: number;
  appliedRevision: number;
  appliedFingerprint?: string;
  phase: 'idle' | 'applying' | 'recovering';
  pendingByRevision: Map<number, PendingPublication>;
}

interface PendingPublication {
  readonly revision: number;
  readonly fingerprint: string;
  readonly input: ProjectPublicationSubmission;
  readonly affectedGraphPaths: ReadonlySet<string>;
  readonly waiters: Array<{
    resolve(value: ProjectPublicationSuccess): void;
    reject(reason: ProjectPublicationError): void;
  }>;
  ownerRecoveryAttempt?: number;
}
```

`submit` must synchronously validate project identity, wire shape, caller validation, revision, and fingerprint before queue mutation. Matching duplicate deliveries append a waiter and return a new promise settled by the same pending publication; they do not start a second prepare/commit. Different fingerprints reject with `publication_protocol_error`. Revisions below the watermark resolve `duplicate` only when the coordinator has the installed fingerprint for that revision; otherwise trigger recovery rather than labeling unknown authority stale.

The drain loop processes only `appliedRevision + 1`. Set `phase = 'applying'`, prepare completely, revalidate identity/epoch/revision, call the synchronous commit, then set `appliedFingerprint`, `appliedRevision`, and success-settle all waiters. If the next revision is absent while higher revisions exist, enter recovery immediately.

- [ ] **Step 4: Split move preparation from the synchronous store commit**

Refactor `migrateGraphResourcePath.ts` into `projectPublicationMovePlan.ts` and remove the async mutation function. Preparation must:

1. validate `from`, `to`, `kind`, `name`, destination projection identity, and destination projection basis;
2. read but not mutate resource metadata and document flags;
3. capture tab placement/registry, graph session/focus, graph metadata, viewport, projection ownership, caller references, and variable scopes;
4. build an immutable `PreparedGraphResourceMove` containing every before/after value needed by commit;
5. reject conflicting destinations, missing source identity, and malformed snapshots before return.

`commitGraphResourceMove` performs, in this exact synchronous order, with no service call and no promise:

```text
projection destination install -> old projection removal -> resource metadata/name move ->
document state move -> graph metadata move -> caller reference cascade -> variable scope move ->
graph session/focus move -> editor tab ID move -> viewport ownership move -> resource loaded mark
```

Every operation uses prevalidated values from the plan. Preserve source `dirty`, `stale`, and `conflict` flags at the destination. Delete the old `migrateGraphResourcePath` export and update its focused tests to call prepare then commit. Keep the exact tests `does not mutate path-owned stores when the target pre-load returns false` and `abandons the post-load publication when the project changes during migration`, but move preload/lifecycle assertions to coordinator tests because the move-plan module performs no I/O.

- [ ] **Step 5: Prepare and commit normal publications atomically**

Move pure validation/preflight code from `resourceMutationResult.ts` into exported pure functions. `prepareSynchronousPublicationCommit(...)` must validate all replacement membership, function before/after revisions, variable before values, move/delta correlation, and destination names before returning a plan.

For each queued publication, collect preparation paths as the union of:

```ts
new Set([
  ...input.fallbackPaths ?? [],
  ...result.projectionStatus.status === 'complete'
    ? result.projectionStatus.expectedGraphPaths
    : result.projectionStatus.invalidatedGraphPaths,
  ...result.moves.map((move) => move.to),
])
```

Fetch every required projection through `dependencies.prepareGraphProjection`; treat `false` as failure. Build every move plan only after its destination projection exists. Recheck identity/epoch after each `await`. `commitPreparedPublication` synchronously commits moves, replacement projections, function installs, and variable installs; then `ProjectPublicationCoordinator` publishes history and advances its watermark last. A failed normal prepare does not retry the mutation; it starts authoritative recovery because Rust may already have committed it.

- [ ] **Step 6: Implement complete authoritative recovery and failure cleanup**

At recovery start, allocate a monotonically increasing attempt ID, capture identity/epoch, snapshot loaded graph paths, and assign every currently pending entry to that attempt. While recovery awaits, newly queued entries join the same attempt but cannot apply.

After `loadRecoverySnapshot()` resolves, recheck current identity/epoch and re-read `state.appliedRevision` before deciding what remains missing. Validate snapshot identity, nonnegative safe publication revision, required history, graph metadata/function signatures, variables, and worksheets. Build recovery paths from:
Build recovery paths from the authoritative index and queued moves:

1. Build a `from -> to` move chain from queued publications and reject cycles or conflicting destinations.
2. Rewrite every loaded-at-start path through that chain.
3. Add every queued complete `expectedGraphPath`, incomplete `invalidatedGraphPath`, and move destination after applying the same chain.
4. Intersect the result with graph paths present in the recovery index. A path absent from the authoritative index represents a removed or renamed source and must be reconciled out of tabs/sessions/resources rather than hydrated.

This prevents a loaded pre-rename source path from making recovery fail after the backend has moved it. Prepare every remaining authoritative projection first. Build `PreparedProjectRecovery` from the index and projection map without mutating stores. After final identity/epoch revalidation, `commitRecovery` synchronously reconciles resource metadata/order, variables, worksheets, function metadata, graph projections, document/session/tab ownership, and open tabs; then installs snapshot history and sets `appliedRevision = snapshot.publicationRevision` last.

Resolve pending revisions `<= snapshot.publicationRevision` with `{ status: 'recovered' }` and do not call normal commit for them. Clear their entries and resume normal draining only for higher contiguous revisions.

On any snapshot or projection failure, create one `ProjectPublicationError('publication_recovery_failed', ...)`, reject every waiter whose `ownerRecoveryAttempt` matches, remove those entries and fingerprints, set `phase = 'idle'`, preserve the pre-attempt history/watermark, and call `markProjectProjectionStale()` once. Do not auto-retry. A later `submit` may allocate a new recovery attempt.

- [ ] **Step 7: Wire lifecycle cancellation and the production singleton**

In `projectClientReset.ts`, call `projectPublicationCoordinator.cancelProject()` before clearing any project-owned store. `cancelProject` increments epoch synchronously, rejects all queued/applying/recovering waiters with `ProjectPublicationError('stale_project_lifecycle', ...)`, clears all pending/recovery references, resets identity/watermark, and makes every old async continuation fail revalidation.

In `projectIOStore.loadProject`, after the authoritative index and project-owned stores are synchronously installed, call:

```ts
projectPublicationCoordinator.startProject(
  index.projectInstanceId,
  index.publicationRevision,
);
```

`startProject` rejects any previous lifecycle first, then establishes the new identity, epoch, and baseline. Remove `setResourceMutationProjectInstanceId`, `resetResourceMutationPublicationState`, `getResourceMutationProjectInstanceId`, and all callers. Resource-index refresh validates against `useProjectIOStore.getState().projectInstanceId`; it does not own publication state.

Add a raw preparation API to `graphProjectionCoordinator.ts` that obtains an `EditorGraphProjectionDto` with lifecycle ownership but does not call graph/resource store setters. The publication coordinator owns the eventual synchronous installation. Existing interactive load/hydrate APIs may continue to install their own user-requested projections, but resource publication application must not call those mutating APIs during prepare.

- [ ] **Step 8: Delegate every direct/event path and remove alternate owners**

Update `ResourceMutationCommittedHandler`, function signature/history coordinators, rename resource action, worksheet create/delete flows, and any remaining direct caller to use:

```ts
await projectPublicationCoordinator.submit({
  result,
  fallbackPaths,
  validate: callerValidation,
});
```

The event handler must not suppress a matching direct result before submission; direct/event deduplication belongs to the coordinator. Pending operation correlation may suppress legacy `GraphDelta` handling, but never `ResourceMutationCommitted` coordinator delivery. On `publication_protocol_error` or `publication_recovery_failed`, log/request UI recovery through the coordinator state only; do not independently hydrate affected paths or install history.

Delete the old `resourceMutationResult.test.ts` after its still-valid wire validation cases are transferred to `projectPublicationCoordinator.test.ts`. Verify no production import references `applyResourceMutationResultWithMoves`, `validateAndApplyResourceMutationResult`, `migrateGraphResourcePath`, `setResourceMutationProjectInstanceId`, or `resetResourceMutationPublicationState`.

- [ ] **Step 9: Run Task 2 GREEN checks**

Run sequentially:

```sh
pnpm exec vitest run src/features/application/editorMutation/projectPublicationCoordinator.test.ts
pnpm exec vitest run src/features/application/editorMutation/projectPublicationMovePlan.test.ts
pnpm exec vitest run src/features/application/editorMutation/historyCoordinator.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts
pnpm exec vitest run src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts src/features/application/resource/resourceActions.test.ts
pnpm exec vitest run src/features/core/dataStore/projectIOStore.test.ts src/features/application/graphDocument/functionSignatureSync.test.ts
pnpm typecheck
git diff --check
```

Expected: all named coordinator behaviors and explicit integration-adjacent files pass, TypeScript typecheck passes, and no whitespace errors are reported. Do not run an unqualified Vitest command, any Rust suite, or any verify script.

---

### Task 3: Integrate the sole owner, preserve/repair report evidence, and re-review filesystem Task 2

**Files:**
- Create: `src/features/application/editorMutation/projectPublicationIntegration.test.ts`
- Modify: `src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts`
- Modify: `src/features/application/editorMutation/historyCoordinator.test.ts`
- Modify: `src/features/application/editorMutation/functionSignatureCoordinator.test.ts`
- Modify: `src/features/application/resource/resourceActions.test.ts`
- Modify: `src/features/core/dataStore/projectIOStore.test.ts`
- Create by byte-for-byte copy: `.superpowers/sdd/2026-07-25-revisioned-mutation-history/task-2-report.pre-publication-recovery-repair.md`
- Create by byte-for-byte copy: `.superpowers/sdd/2026-07-27-project-filesystem-transaction/task-2-report.pre-publication-recovery-repair.md`
- Create: `.superpowers/sdd/2026-07-27-project-filesystem-transaction/project-publication-recovery-report.md`
- Create: `.superpowers/sdd/2026-07-27-project-filesystem-transaction/project-publication-recovery-review-package.md`
- Modify append-only: `.superpowers/sdd/2026-07-25-revisioned-mutation-history/task-2-report.md`
- Modify append-only: `.superpowers/sdd/2026-07-27-project-filesystem-transaction/task-2-report.md`
- Modify append-only: `.superpowers/sdd/2026-07-27-project-filesystem-transaction/progress.md`
- Modify: `docs/superpowers/plans/2026-07-27-project-filesystem-transaction.md` (replace downstream references to deleted `migrateGraphResourcePath*` files with `projectPublicationMovePlan*` ownership)
- Review: `docs/superpowers/plans/2026-07-27-project-filesystem-transaction.md:181-347`
- Review: `.superpowers/sdd/2026-07-27-project-filesystem-transaction/task-2-brief.md`
- Review: `.superpowers/sdd/2026-07-27-project-filesystem-transaction/task-2-review-package.md`

**Interfaces:**
- Consumes: Tasks 1–2 `projectPublicationCoordinator`, all resource mutation direct/event callers, the approved recovery spec, the filesystem Task 2 brief/report/review package, and both existing dated Task 2 reports.
- Produces: one source-audit test proving sole ownership; immutable pre-repair copies of both report artifacts; one recovery execution report/review package; append-only provenance corrections; and an explicit `CLEAN` or `BLOCKED` whole-Task-2 re-review decision.
- The filesystem plan may resume Task 3 only if the final ledger line is exactly:

```text
Task 2: complete (publication recovery sub-plan passed; whole-Task-2 re-review clean; Task 3 gate reopened; no commits by plan constraint)
```

If any correctness finding remains, append a `Task 2: BLOCKED` line that quotes the highest-severity review finding verbatim, states that Task 3 and later remain undispatched, and states that the full Rust suite was not run. Never use a generic summary or mark the gate clean while a finding is open.

- [ ] **Step 1: Add the failing sole-owner source audit**

Create `projectPublicationIntegration.test.ts` with this exact test:

```ts
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const productionFiles = [
  'src/features/core/sync/handlers/ProjectMutationEventHandler.ts',
  'src/features/application/editorMutation/functionSignatureCoordinator.ts',
  'src/features/application/editorMutation/historyCoordinator.ts',
  'src/features/application/resource/resourceActions.ts',
  'src/features/application/editor/closeEditorTab.ts',
  'src/features/application/editor/useWorksheetManagement.ts',
  'src/features/core/dataStore/projectIOStore.ts',
];

describe('project publication integration boundary', () => {
  it('leaves one publication owner and no legacy applier or move path', () => {
    const sources = productionFiles.map((path) =>
      [path, readFileSync(resolve(path), 'utf8')] as const,
    );
    const joined = sources.map(([path, source]) => `${path}\n${source}`).join('\n');

    expect(joined).not.toMatch(/applyResourceMutationResultWithMoves/);
    expect(joined).not.toMatch(/validateAndApplyResourceMutationResult/);
    expect(joined).not.toMatch(/migrateGraphResourcePath/);
    expect(joined).not.toMatch(/setResourceMutationProjectInstanceId/);
    expect(joined).not.toMatch(/resetResourceMutationPublicationState/);
    for (const [path, source] of sources.slice(0, 6)) {
      expect(source, path).toContain('projectPublicationCoordinator.submit');
    }
    expect(sources[6][1]).toContain('projectPublicationCoordinator.startProject');
    expect(sources[6][1]).not.toMatch(/latestPublicationRevision|publicationDrain|authoritativeGapRecovery/);
  });
});
```

- [ ] **Step 2: Run the exact audit and verify RED**

Run:

```sh
pnpm exec vitest run src/features/application/editorMutation/projectPublicationIntegration.test.ts
```

Expected: FAIL while any legacy applier/move/lifecycle symbol remains or any named direct/event caller does not delegate to `projectPublicationCoordinator.submit`.

- [ ] **Step 3: Finish integration tests and remove the final alternate path**

Update the existing handler/coordinator/action tests to assert:

- event-first and direct-first matching deliveries settle through one coordinator commit;
- reverse arrival remains revision ordered through the real event handler;
- no handler performs fallback graph hydration after coordinator recovery failure;
- function/history coordinators do not install history independently;
- rename/undo/redo destination names survive through real store commits;
- project reset rejects old event/direct promises before any replacement-project store write;
- index load starts the coordinator at `index.publicationRevision` and `index.history` is installed as the baseline.

Remove any production symbol caught by the audit rather than excluding a file or weakening the regex. Keep `GraphDeltaHandler` only for graph-mutation projection invalidation; it must not update the resource publication watermark or history.

Update the remaining filesystem transaction plan so Tasks 8–9 and their explicit test lists refer to `projectPublicationMovePlan.ts` / `projectPublicationMovePlan.test.ts` instead of the deleted `migrateGraphResourcePath.ts` / `migrateGraphResourcePath.test.ts`. Do not change the approved behavior or reopen completed Task 1 requirements; this is a dependency-path correction only.

- [ ] **Step 4: Run the explicit integration GREEN set**

Run:

```sh
pnpm exec vitest run src/features/application/editorMutation/projectPublicationIntegration.test.ts
pnpm exec vitest run src/features/application/editorMutation/projectPublicationCoordinator.test.ts src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts
pnpm exec vitest run src/features/application/editorMutation/historyCoordinator.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/application/resource/resourceActions.test.ts
pnpm exec vitest run src/features/core/dataStore/projectIOStore.test.ts src/features/application/editorMutation/projectPublicationMovePlan.test.ts
pnpm typecheck
git diff --check
```

Expected: all explicit files pass, typecheck passes, and the diff has no whitespace errors. Do not run unqualified Vitest, a full frontend suite, or any verify script.

- [ ] **Step 5: Preserve both pre-repair reports byte-for-byte**

Before editing either dated `task-2-report.md`, copy rather than rewrite:

```text
.superpowers/sdd/2026-07-25-revisioned-mutation-history/task-2-report.md
-> .superpowers/sdd/2026-07-25-revisioned-mutation-history/task-2-report.pre-publication-recovery-repair.md

.superpowers/sdd/2026-07-27-project-filesystem-transaction/task-2-report.md
-> .superpowers/sdd/2026-07-27-project-filesystem-transaction/task-2-report.pre-publication-recovery-repair.md
```

Verify both copies are byte-identical with:

```sh
git --no-pager diff --no-index -- .superpowers/sdd/2026-07-25-revisioned-mutation-history/task-2-report.md .superpowers/sdd/2026-07-25-revisioned-mutation-history/task-2-report.pre-publication-recovery-repair.md
git --no-pager diff --no-index -- .superpowers/sdd/2026-07-27-project-filesystem-transaction/task-2-report.md .superpowers/sdd/2026-07-27-project-filesystem-transaction/task-2-report.pre-publication-recovery-repair.md
```

Expected: both commands exit 0 with no diff. If either differs, recreate only the new copy; do not alter the source report.

- [ ] **Step 6: Write the recovery report and append provenance repairs**

Create `project-publication-recovery-report.md` with these populated sections: `Status`, `Scope`, `Task 1 RED/GREEN`, `Task 2 RED/GREEN`, `Task 3 integration`, `Exact commands and outputs`, `Files changed`, `No-full-suite declaration`, and `Remaining concerns`. Record actual command output and counts only; do not claim a pass without fresh output.

Append—never replace or delete—this provenance section to the 2026-07-25 report:

```markdown
## Artifact provenance repair (2026-07-27 project-publication recovery)

The pre-repair file is preserved byte-for-byte at
`task-2-report.pre-publication-recovery-repair.md`. This report remains the
2026-07-25 revisioned-mutation-history Task 2 evidence. Filesystem Task 2
publication-recovery evidence belongs to the 2026-07-27 workspace and is
linked from
`../2026-07-27-project-filesystem-transaction/project-publication-recovery-report.md`.
No historical RED/GREEN evidence above was deleted or rewritten.
```

Append this section to the 2026-07-27 report, filling the bracketed status with the actual review decision `CLEAN` or `BLOCKED` before writing:

```markdown
## Publication recovery supersession and artifact repair

The pre-repair file is preserved byte-for-byte at
`task-2-report.pre-publication-recovery-repair.md`. The five-round frontend
publication loop above is superseded by the approved focused plan
`docs/superpowers/plans/2026-07-27-project-publication-recovery.md`; it is
retained as historical evidence and is not a completion claim. Fresh recovery
evidence is recorded in `project-publication-recovery-report.md`.

The mismatched 2026-07-25/2026-07-27 Task 2 report provenance is repaired by
keeping each dated report in its owning plan workspace, preserving both
pre-repair files, and cross-linking the recovery evidence instead of moving or
overwriting historical sections.

After this section, append either `### Whole-Task-2 re-review: CLEAN` or
`### Whole-Task-2 re-review: BLOCKED`, matching the fresh review package. Task
3 remains gated unless the recorded status is **CLEAN** and the ledger contains
the exact clean completion line required by the recovery plan.
```

- [ ] **Step 7: Build the focused review package and perform the mandatory whole-Task-2 re-review**

Create `project-publication-recovery-review-package.md` containing:

1. the approved design and this plan paths;
2. `git --no-pager diff --` output limited to the production/test files named in Tasks 1–3;
3. every exact RED observation and GREEN command/result;
4. the sole-owner source-audit result;
5. both pre-repair report copy paths and byte-identity command results;
6. a checklist mapping all ten approved-spec test bullets to concrete passing tests;
7. explicit confirmation that no compatibility event, alternate applier, dual watermark, automatic mutation replay, full suite, verify command, or commit was used.

Re-review the complete filesystem Task 2 against lines 181–347 of its main plan, its brief, all five historical rounds, the recovery report, and the focused diff. Record findings with severity, exact file/symbol, reproduction, and required correction. Do not mark `CLEAN` while any publication ordering, preparation atomicity, recovery settlement, lifecycle cancellation, authoritative-name, backend-baseline, or original filesystem Task 2 finding remains open.

- [ ] **Step 8: Append the ledger decision without erasing the blocker history**

Keep the existing lines 8–14 in `.superpowers/sdd/2026-07-27-project-filesystem-transaction/progress.md`. Append:

```text
Task 2: recovery sub-plan — backend baseline, coordinator, integration, and evidence-preserving report repair completed with focused checks; no commits; full Rust suite not run.
```

If and only if the re-review is clean, append the exact clean completion line from the Interfaces block. Otherwise append the exact blocked form with the concrete highest-severity open finding. Do not dispatch or edit any Task 3 production file in this sub-plan.

- [ ] **Step 9: Run final focused document/source checks**

Run sequentially:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- commands::command_project::query::tests::project_index_carries_one_coherent_publication_recovery_baseline --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- event::event_project::tests::resource_mutation_result_serializes_explicit_graph_move_identity --exact --test-threads=1
pnpm exec vitest run src/features/application/editorMutation/projectPublicationCoordinator.test.ts src/features/application/editorMutation/projectPublicationIntegration.test.ts
pnpm exec vitest run src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts src/features/application/editorMutation/historyCoordinator.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts src/features/application/resource/resourceActions.test.ts
pnpm exec vitest run src/features/core/dataStore/projectIOStore.test.ts src/features/application/editorMutation/projectPublicationMovePlan.test.ts
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
pnpm typecheck
git diff --check
```

Expected: both focused Rust tests pass with single-job/single-thread limits, every explicit Vitest file passes, Rust/TypeScript checks pass, formatting/whitespace checks pass, both preserved report copies exist, and the ledger states either a truthful clean gate or a truthful continuing blocker. Do not run a full Rust suite, unqualified frontend suite, or any verify command.
