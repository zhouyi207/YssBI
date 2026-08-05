# History Survives Graph Cache Unload Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve project-scoped Rust Undo/Redo across graph cache unload and atomically apply History transactions to unloaded persisted graph resources without reloading them into `ProjectState.project_data`.

**Architecture:** Keep the existing `ProjectHistory` and patch engine. Direct and lifecycle unload preserve the History stacks; History execution uses a new internal preparation layer to snapshot the head and residency, hydrate missing graph-owned documents outside locks, apply the transaction to a temporary document state, then either retain the current loaded-only path or commit a mixed loaded/unloaded result through the existing project filesystem transaction with commit-time authority validation.

**Tech Stack:** Rust, Tauri project domain, `ProjectHistory`, `ProjectDocumentState`, project filesystem transactions, serde graph documents, focused serial Cargo tests.

## Global Constraints

- Work directly on branch `shadcn`; do not create a worktree, branch, commit, or tag.
- Preserve unrelated dirty work.
- `ProjectState.project_data` remains authoritative for loaded project/graph/pin state; disk remains authoritative for unloaded persisted graph documents.
- Reuse the existing `ProjectHistory`, patch engine, revision ledgers, `ProjectFilesystemTransaction`, publication DTOs, and recovery-required gate; do not create a second History or filesystem engine.
- Graph unload is a cache/lifecycle operation and creates no History transaction.
- Project reload or replacement activation remains the boundary that clears History.
- An unloaded graph must remain absent from `project_data` after Undo/Redo.
- Graph/function/local-variable resources are identified only by stable resource paths and IDs; never bind by labels or insertion order.
- Revisions remain monotonic; Undo/Redo never restores an old revision number.
- Do not hold project-wide locks during filesystem reads, staging, sleeps, or waits.
- Cross-resource History changes spanning loaded and unloaded resources commit all-or-nothing.
- Use RED-GREEN TDD and focused serial Rust tests with `CARGO_BUILD_JOBS=1` and `--test-threads=1`.
- Update `.superpowers/sdd/2026-08-03-history-survives-graph-cache-unload/progress.md` and `TODO.md` after every independently reviewed task. Keep Phase 4 at 99% until final whole-slice review and fresh verification pass.
- Existing 18 `unused_must_use` warnings in `src-tauri/src/project/production_tests.rs` and LF-to-CRLF notices are non-failing pre-existing warnings.

## File structure

- Modify `src-tauri/src/project/project_state.rs`: route unload through History-preserving behavior and delegate History execution to the preparation/commit boundary.
- Create `src-tauri/src/project/history_hydration.rs`: own touched-resource discovery, residency snapshots, lock-free disk hydration, temporary document-state construction, and durable History preparation types.
- Modify `src-tauri/src/project/mod.rs`: register the private `history_hydration` module.
- Modify `src-tauri/src/project/project_io.rs` only if a narrow canonical graph-document serializer/reader must become `pub(crate)` for hydration; do not duplicate its format logic.
- Modify `src-tauri/src/project/production_tests.rs`: production-level unload, disk Undo/Redo, mixed-residency, Function/local-variable, publication, and reload tests.
- Modify `src-tauri/src/project/project_lifecycle.rs` or its existing test module only for lifecycle-token unload regression coverage.
- Modify `src-tauri/src/project/filesystem/tests.rs` only when an existing fault hook needs direct transaction-level proof; prefer production tests for end-to-end behavior.
- Modify `TODO.md`: publish reviewed progress and final Phase 4 completion.

---

### Task 1: Preserve History across both graph unload paths

**Files:**
- Modify: `src-tauri/src/project/project_state.rs:2920-2950`
- Modify: `src-tauri/src/project/project_state.rs:3697-3758`
- Test: `src-tauri/src/project/production_tests.rs`
- Test: `src-tauri/src/project/project_lifecycle.rs`
- Update: `TODO.md`

**Interfaces:**
- Consumes: `ProjectState::history_status`, `ProjectHistory::undo_len`, `ProjectHistory::redo_len`, direct unload, lifecycle unload.
- Produces: both unload methods preserve the exact History stack/head while retaining current residency, revision-ledger, compile-invalidation, and stale lifecycle behavior.

- [ ] **Step 1: Add a direct-unload failing production test**

Add `graph_cache_unload_preserves_complete_project_history` in `production_tests.rs`. Build two undoable transactions on different resources, record status and head lengths through test-only accessors, persist the graph being unloaded, call `unload_graph_resource`, and assert:

```rust
assert!(!state.get_data().unwrap().graphs.contains_key(&unloaded));
assert_eq!(state.history_status(), before_status);
assert_eq!(state.history_lengths_for_test(), before_lengths);
assert_eq!(state.history_head_id_for_test(true), before_head);
```

Also assert compile invalidation and graph/local-variable eviction retain their existing behavior.

- [ ] **Step 2: Run the direct-unload test and verify RED**

Run:

```sh
CARGO_BUILD_JOBS=1 cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib graph_cache_unload_preserves_complete_project_history -- --exact --nocapture --test-threads=1
```

Expected: FAIL because `unload_graph_resource` clears both History stacks.

- [ ] **Step 3: Add lifecycle unload and stale-token failing coverage**

Add tests proving successful `unload_graph_resource_for_lifecycle` preserves History and stale project/token rejection leaves History and residency unchanged. Use exact project instance and monotonically newer lifecycle tokens; do not bypass `GraphLifecycleCoordinator`.

- [ ] **Step 4: Run lifecycle tests and verify RED**

Run the exact new tests serially. Expected: successful unload test fails on History status/head; stale rejection remains zero-effect.

- [ ] **Step 5: Preserve History and align no-op unload authority semantics**

Delete the two unload-path calls to:

```rust
self.history.write().unwrap().clear();
```

Make direct unload compute the same `changed = graph_removed || variables_removed` fact as lifecycle unload. Advance authority generation and invalidate compile products only when `changed` is true. Add an already-unloaded direct-call assertion proving generation, History, revisions, and residency remain unchanged. Do not change project activation/reload History reset and do not record unload as a transaction.

- [ ] **Step 6: Run GREEN and regression coverage**

Run the new direct/lifecycle tests plus:

```sh
CARGO_BUILD_JOBS=1 cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project_reload_clears_history_status -- --exact --nocapture --test-threads=1
CARGO_BUILD_JOBS=1 cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib lifecycle_unload_retains_exact_graph_revision -- --exact --nocapture --test-threads=1
```

Expected: unload preserves History; reload still clears it; graph revision behavior is unchanged.

- [ ] **Step 7: Verify and publish reviewed progress**

Run `pnpm rust:fmt:check` and `git diff --check`. After independent task review, append Task 1 evidence to the slice ledger and update the Phase 4 row in `TODO.md` to state that unload preservation is complete while disk hydration remains; keep it at 99%.

---

### Task 2: Prepare and hydrate an unloaded History head outside locks

**Files:**
- Create: `src-tauri/src/project/history_hydration.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/project/project_state.rs:4209-4332`
- Modify: `src-tauri/src/project/project_io.rs:397-431` only if narrow visibility is required
- Test: `src-tauri/src/project/production_tests.rs`
- Test: unit tests inside `src-tauri/src/project/history_hydration.rs`
- Update: `TODO.md`

**Interfaces:**
- Consumes: cloned `ProjectHistoryTransaction`, `HistoryEntryId`, `ProjectSession`, publication authority generation, graph/variable revision snapshots, `ProjectFilesystemCoordinator::acquire`, `project_io::load_project_graph_document_from_file`, existing `project_documents` representation.
- Produces:

```rust
pub(super) struct HistoryPreparationBasis {
    pub session: ProjectSession,
    pub authority_generation: u64,
    pub history_id: HistoryEntryId,
    pub undo: bool,
    pub expected_revisions: BTreeMap<ResourceKey, ResourceRevision>,
    pub residency: BTreeMap<GraphResourcePath, HistoryGraphResidency>,
}

pub(super) enum HistoryGraphResidency {
    Loaded,
    Unloaded,
}

pub(super) struct PreparedHistoryDocuments {
    pub lease: ProjectFilesystemLeaseSet,
    pub basis: HistoryPreparationBasis,
    pub before: ProjectDocumentState,
    pub after: ProjectDocumentState,
    pub transaction: ProjectHistoryTransaction,
    pub touched_graphs: BTreeSet<GraphResourcePath>,
    pub contains_unloaded_graph: bool,
}
```

Names may be made more focused during implementation, but the module must own the same facts and must not persist residency metadata.

- [ ] **Step 1: Add pure touched-resource/residency failing tests**

Test that the preparation module:

- resolves Graph resources directly;
- maps Function resources to their owning Function graph path;
- maps local Variable resources through authoritative scope to the owning graph;
- leaves Global variables project-scoped;
- deduplicates one graph touched through Graph + Function + local Variable patches;
- rejects a Function/local variable whose owning graph cannot be resolved.

Use stable resource IDs and exact opaque paths in fixtures.

- [ ] **Step 2: Run the preparation tests and verify RED**

Run:

```sh
CARGO_BUILD_JOBS=1 cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib history_hydration::tests -- --nocapture --test-threads=1
```

Expected: compilation/test failure because the module and preparation types do not exist.

- [ ] **Step 3: Implement touched-resource discovery and immutable preparation basis**

Implement pure helpers that inspect `ProjectHistoryTransaction::changes`, loaded `ProjectData`, variable scope/revision facts, and graph revision ledger. The snapshot must contain only cloned data needed after locks release. It must capture `HistoryEntryId`, direction, project session, authority generation, exact expected revision keyed by every touched `ResourceKey` (Graph, Function, Variable including tombstones), and exact loaded/unloaded graph residency. Function revision is the revision inside the Function document embedded in its owning graph; do not create a Function revision ledger. For an already-unloaded local variable, derive its owner from the current History patch's present-side scoped value and later verify it against the hydrated graph document; never scan arbitrary graph files.

- [ ] **Step 4: Add failing disk-hydration production coverage**

Add `unloaded_graph_history_preparation_hydrates_disk_without_loading_cache`:

1. create and persist a graph;
2. record an ordinary Graph patch transaction;
3. persist the edited state and unload the graph;
4. prepare Undo;
5. assert preparation contains the graph document and graph-owned local variables;
6. assert `project_data.graphs` still excludes it;
7. assert a missing/corrupt graph returns a structured error with History, revisions, publication, and disk unchanged.

- [ ] **Step 5: Run hydration coverage and verify RED**

Expected: current `commit_history_direction` reports that the History anchor is absent because `project_documents` contains loaded resources only.

- [ ] **Step 6: Implement lock-free hydration**

After releasing project locks, acquire the project-root `ProjectFilesystemLeaseSet` before the first hydration read. Use the canonical project graph reader to load absent touched graph documents while that coordinator lease is held. Move the lease into `PreparedHistoryDocuments` and retain it through staging, disk commit, authority validation, finalization, or rollback. Merge loaded snapshots and hydrated graph-owned documents into a temporary `ProjectDocumentState`. Apply Undo/Redo to cloned History/document state during preparation. Do not mutate live History yet and do not insert hydrated graphs into `project_data`.

- [ ] **Step 7: Prove immutable basis comparison facts**

Add pure tests showing basis comparison detects changed History head, Graph revision, Function revision, variable/tombstone revision, authority generation, project instance, and graph residency. Production-wired checkpoints and rollback proofs belong to Task 3 after the durable commit boundary exists.

- [ ] **Step 8: Run GREEN and focused document tests**

Run the new preparation/hydration tests and:

```sh
CARGO_BUILD_JOBS=1 cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib node_system::document::tests -- --nocapture --test-threads=1
```

Expected: all pass; existing patch validation remains authoritative.

- [ ] **Step 9: Verify and publish reviewed progress**

Run `pnpm rust:check`, `pnpm rust:fmt:check`, and `git diff --check`. After independent review, update ledger/TODO; keep Phase 4 at 99% because durable commit is not complete.

---

### Task 3: Atomically commit mixed loaded/unloaded History transactions

**Files:**
- Modify: `src-tauri/src/project/history_hydration.rs`
- Modify: `src-tauri/src/project/project_state.rs:4209-4332`
- Modify: `src-tauri/src/project/project_io.rs:182-192` only to expose an existing serializer, never duplicate graph JSON construction
- Test: `src-tauri/src/project/production_tests.rs`
- Test: `src-tauri/src/project/filesystem/tests.rs` only for missing reusable fault proof
- Update: `TODO.md`

**Interfaces:**
- Consumes: lease-owning `PreparedHistoryDocuments`, `ProjectFilesystemTransaction::prepare_with_validator`, `StagedFilesystemMutation::Write`, `CommittedFilesystemMutation`, existing graph serializer, publication/revision locks.
- Produces: policy-specific routing that preserves existing `DurableVariableEffects` and `DurableResourceMove` workflows; only `InMemoryUntilSave` plus `contains_unloaded_graph` selects the new hydration transaction, while loaded-only `InMemoryUntilSave` retains the current in-memory path.

- [ ] **Step 1: Add failing unloaded Graph Undo/Redo production test**

Add `unloaded_graph_edit_undo_redo_is_durable_and_keeps_graph_unloaded`:

- edit and persist Graph A;
- unload A without clearing History;
- Undo using the current resource revision;
- read A from disk and verify the inverse content;
- assert A remains absent from `project_data` and has no projection replacement;
- Redo and verify forward content on disk;
- assert each delta revision strictly increases;
- after Undo and Redo, assert serialized document revision equals graph revision ledger revision and the corresponding delta `to_revision`, while each delta `from_revision` equals the actual prior authority revision.

- [ ] **Step 2: Run the test and verify RED**

Expected: preparation can hydrate after Task 2, but no durable write-back path exists.

- [ ] **Step 3: Add failing mixed-residency atomicity test**

Create one `ProjectHistoryTransaction` touching loaded Graph A and persisted/unloaded Graph B. Undo and Redo must update both documents, publish deltas for both, produce a projection only for A, and leave B unloaded. Assert exact before/after disk and memory documents. Add a second transaction touching an unloaded graph and a Global variable, proving graph file and project-level variable persistence commit or roll back together.

- [ ] **Step 4: Implement durable staging without global locks**

Serialize every durable document affected by the transaction through canonical serializers: graph files include Function/local-variable state, while Global variables use the existing project-level persistence path. Move the lease already held by `PreparedHistoryDocuments` into `ProjectFilesystemTransaction::prepare_with_validator`; do not release and reacquire it between hydration and staging. Perform all staging outside project data/History/publication locks and validate every staged document with its canonical parser.

Existing `DurableVariableEffects` and `DurableResourceMove` transactions keep their current specialized workflows. Loaded-only `InMemoryUntilSave` transactions bypass the new filesystem path.

- [ ] **Step 5: Implement commit-time revalidation and all-or-nothing authority commit**

At commit, revalidate:

```text
project instance
project root/session
recovery gate
HistoryEntryId and direction
authority generation
touched revisions
loaded/unloaded residency
```

Before touching live authority, precompute cloned after-state, deltas, loaded projection basis, compile invalidation, and every fallible conversion. Then:

1. commit the prepared filesystem transaction outside all project authority locks, leaving its rollback guard armed;
2. pause at a bounded test checkpoint that can race a new mutation/residency change;
3. acquire authority locks and revalidate every basis fact;
4. on mismatch, release locks and roll the committed filesystem mutation back;
5. on success, perform one non-fallible authority swap that moves History, updates only originally loaded `project_data` (including loaded Function documents), keeps unloaded graphs absent, advances Graph and Variable revision ledgers, invalidates compile products, and allocates one publication revision; Function revision remains embedded in its owning graph document and never receives a separate ledger;
6. release locks and finalize the filesystem mutation;
7. publish precomputed deltas and projections only after finalization.

No authority lock may be held during filesystem commit, rollback, or finalize. If rollback fails, enter the existing recovery-required gate.

- [ ] **Step 6: Run unloaded and mixed GREEN tests**

Run both exact production tests serially. Expected: durable disk changes, monotonic revisions, correct projections, unchanged residency.

- [ ] **Step 7: Add fault-injection tests**

Cover:

- hydration read failure;
- `ProjectFilesystemFaultPoint::StagedSerialization`;
- first live replacement failure with successful rollback;
- rollback restore failure entering recovery-required.

For each pre/recoverable failure, snapshot and compare History head/status, loaded data, Graph/Function/variable revisions and tombstone presence, publication revision/generation, and every touched file. No completion/projection may publish. Ensure all History replay, conversion, serialization, delta construction, and projection-basis work happens before live authority assignment so post-filesystem validation has only rollback or one non-fallible authority swap.

- [ ] **Step 8: Add bounded race tests**

Use channels/test hooks, not sleeps, at both after-preparation and after-disk-commit/before-authority-validation checkpoints. Change the History head, Graph revision ledger, loaded Function document revision, variable/tombstone revision, authority generation, project instance, and graph residency in focused tests. For an unloaded Function, start a second coordinator-backed filesystem operation and prove it cannot enter between hydration and finalization while the prepared execution owns the lease; arbitrary external writes that bypass the coordinator are outside this transaction guarantee. Commit must reject stale preparation, roll disk back when already committed, and leave the newer authority untouched.

- [ ] **Step 9: Run focused filesystem/history suites**

Run:

```sh
CARGO_BUILD_JOBS=1 cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib unloaded_graph_ -- --nocapture --test-threads=1
CARGO_BUILD_JOBS=1 cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib mixed_residency_ -- --nocapture --test-threads=1
CARGO_BUILD_JOBS=1 cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::filesystem::tests -- --nocapture --test-threads=1
```

Expected: all pass with no active transaction staging directories left behind.

- [ ] **Step 10: Verify and publish reviewed progress**

Run `pnpm rust:check`, `pnpm rust:fmt:check`, and `git diff --check`. After independent review, update ledger/TODO; retain Phase 4 at 99% pending graph-owned resource and final integration coverage.

---

### Task 4: Prove Function/local-variable coherence and finalize Phase 4

**Files:**
- Modify: `src-tauri/src/project/history_hydration.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Test: `src-tauri/src/project/production_tests.rs`
- Test: `src-tauri/src/commands/command_node_system.rs` only if command publication coverage requires fixture updates
- Modify: `TODO.md`
- Update: `.superpowers/sdd/2026-08-03-history-survives-graph-cache-unload/progress.md`

**Interfaces:**
- Consumes: completed History hydration/durable commit path, Function graph ownership, local variable scope, presence tombstones, `CommittedResourceMutation::complete`.
- Produces: production proof that unloaded Function/local-variable resources remain coherent and canonical publication/reload semantics are unchanged.

- [ ] **Step 1: Add failing Function graph History test**

Persist and unload a Function graph whose History head changes its `FunctionDocument`. Undo/Redo must preserve exact function resource path, parameter IDs, ABI direction, graph ownership, monotonic revision, and unloaded residency. Hold the project filesystem lease from hydration through finalization so the unloaded Function's embedded revision is validated before replacement. After each direction, assert the serialized Function document revision, the hydrated/resulting Function document revision, and delta `to_revision` agree; do not introduce a Function revision ledger. The callee graph must not appear in `project_data` as a side effect.

- [ ] **Step 2: Add failing graph-local variable History test**

Persist and unload an Event and a Function graph with local variables. Exercise create/update/remove presence patches through Undo/Redo. Verify:

- local variable scope still names the exact owning graph path;
- presence tombstones and variable revision ledgers remain monotonic and equal the corresponding delta/serialized authority revision;
- graph files round-trip the local variables;
- no local variable remains in loaded `project_data` while its graph is unloaded;
- Global variable behavior is unchanged.

- [ ] **Step 3: Run tests and verify RED or missing coverage**

Run exact test names serially. A passing test before implementation is insufficient; use a targeted mutation that removes graph-owned Function/local-variable hydration to prove each test fails for the intended reason, then restore implementation.

- [ ] **Step 4: Complete graph-owned resource hydration/write-back**

Make the smallest changes needed so Function and local-variable documents are sourced and persisted only with their owning graph. Do not introduce standalone Function/local-variable files or caches.

- [ ] **Step 5: Verify canonical publication**

Assert one `ResourceMutationResultDto` contains:

- one operation ID and project instance ID;
- one publication revision;
- deltas for all touched stable resources;
- History status after the committed direction;
- projection replacements only for loaded affected graphs;
- `ProjectionStatusDto::Complete` with exactly the loaded affected graph paths.

Use the bounded after-disk-commit/before-authority-validation checkpoint and an observer channel to prove no event or completion fires before filesystem finalization and the authority swap. Inspecting only the returned DTO is not sufficient.

- [ ] **Step 6: Reconfirm reload and lifecycle boundaries**

Run production tests proving:

- direct unload preserves History;
- lifecycle unload preserves History;
- stale unload has zero effect;
- replacement project activation clears History;
- recovery-required blocks History until activation.

- [ ] **Step 7: Run the focused Phase 4 matrix**

Run serial focused filters for document History, production History/unload, lifecycle, filesystem/recovery, resource mutations, and command publication. Record exact test counts in the ledger.

- [ ] **Step 8: Run project checks**

Run:

```sh
pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

If changes span command/frontend contracts, also run `pnpm typecheck` and the relevant frontend History tests. Do not run a full Rust suite by default unless focused coverage cannot close the risk.

- [ ] **Step 9: Independent final whole-slice review**

The reviewer must verify unload/reload boundaries, no I/O under global locks, History-head/residency/revision revalidation, atomic mixed persistence, Function/local-variable ownership, recovery behavior, publication ordering, and absence of a second authority.

- [ ] **Step 10: Final verification and progress publication**

After a clean final review and fresh controller verification, update `TODO.md` Phase 4 from 99% to 100%. State explicitly that History remains process-local and resets on project reload; do not claim cross-session History persistence.
