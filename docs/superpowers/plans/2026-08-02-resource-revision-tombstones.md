# Resource Revision Tombstones Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve monotonic revision authority across variable deletion/history replay and graph unload without adding a second authority or persistent tombstone format.

**Architecture:** Existing `variable_revisions` and `graph_revisions` maps remain the sole session-scoped ledgers. Revision lookup is decoupled from in-memory document presence; deletion advances and retains variable revisions, while unload retains the unchanged graph revision because it does not mutate graph content.

**Tech Stack:** Rust, `ProjectState`, revisioned resource mutations, project history, graph lifecycle, focused serial Cargo tests through pnpm.

## Global Constraints

- Work directly on `shadcn`; no worktree, branch, or commit.
- Preserve unrelated dirty work, especially `.gitignore`, `TODO.md`, observability, structured-control, relational, and pre-run cancellation slices.
- Rust remains authoritative for documents, revisions, transactions, history, and graph lifecycle.
- Keep `variable_revisions` and `graph_revisions` as the only revision authority; do not add tombstone enums, secondary maps, or persistence fields.
- Tombstones live only for the active project session; activation replaces revision maps from newly activated authoritative documents.
- Variable deletion advances revision; graph unload retains the exact revision without incrementing it.
- `expected_absent_resources` remains based on document/resource presence, not revision-map presence.
- Do not weaken the two existing failing regression tests.
- Use focused serial Rust tests only; do not rerun the known-red complete Rust suite.
- After each task passes independent review and fresh controller verification, immediately update only the relevant rows in `TODO.md` under `## node_architecture 进度`.

---

## File Structure

- Modify `src-tauri/src/project/project_state.rs` for revision validation, history variable removals, and both graph unload paths.
- Modify `src-tauri/src/project/project_state_variable.rs` so the legacy authoritative removal path also retains a monotonically advanced variable tombstone.
- Extend `src-tauri/src/commands/command_variable/mod.rs` to freeze delete/undo/redo revision continuity and stale tombstone rejection.
- Extend `src-tauri/src/project/production_tests.rs` to freeze graph unload revision retention and unloaded cross-resource history behavior.
- Extend `src-tauri/src/project/project_activation.rs` to prove activation replaces old-session tombstones.
- Create `.superpowers/sdd/2026-08-02-resource-revision-tombstones/progress.md` as the durable ledger.

### Task 1: Preserve variable revision authority across deletion and history

**Files:**
- Modify: `src-tauri/src/project/project_state.rs:311-375, 2501-2514`
- Modify: `src-tauri/src/project/project_state_variable.rs:121-139, tests module`
- Modify: `src-tauri/src/commands/command_variable/mod.rs:725-848`
- Create: `.superpowers/sdd/2026-08-02-resource-revision-tombstones/progress.md`
- Modify after review: `TODO.md` under `## node_architecture 进度`

**Interfaces:**
- Consumes: `variable_revisions: HashMap<VariableId, ResourceRevision>` as the session authority.
- Produces: a retained `ResourceRevision` entry when the variable document is absent.
- Preserves: `expected_absent_resources` checks against `ProjectData.variables` only.

- [ ] **Step 1: Record the existing variable RED evidence**

Create the plan ledger and record that the exact history regression fails at undo-delete because expected revision `3` resolves to `missing`:

```markdown
# SDD ledger — plan: docs/superpowers/plans/2026-08-02-resource-revision-tombstones.md

Execution constraints:
- Directly use shadcn; no worktree/branch/commit.
- Existing revision maps remain the only session-scoped authority.
- Focused serial Rust tests only; no complete Rust suite.
- Update TODO.md node_architecture progress after each reviewed task.

Task 1: RED — variable delete/history regression expected revision 3 but validation resolved the deleted document as missing
Task 2: RED — unloaded caller graph expected revision 0 but unload removed graph revision authority
```

- [ ] **Step 2: Add focused variable tombstone assertions before production changes**

In `project_state_variable.rs`, add a test using `active_state`, `add_int_variable`, `remove_variable`, and `revision_state_for_test`:

```rust
#[test]
fn remove_variable_retains_next_revision_tombstone() {
    let state = active_state("remove-tombstone");
    let variable = add_int_variable(&state);
    assert_eq!(
        state.revision_state_for_test().1.get(&variable.id),
        Some(&ResourceRevision::INITIAL)
    );

    state.remove_variable(&variable.id).unwrap();

    assert!(state.get_variable(&variable.id).unwrap().is_none());
    assert_eq!(
        state.revision_state_for_test().1.get(&variable.id),
        Some(&ResourceRevision::new(1))
    );
}
```

In the existing global create/update/delete history test, add assertions after delete, after undo-all, and after redo-all that the document is absent where expected but the revision ledger contains revisions `3`, `6`, and `9`. Before the correct undo-delete request, issue one request with revision `2`, assert exact `MutationConflict::StaleRevision { base_revision: 2, current_revision: 3 }`, and assert it emits no publication. Preserve existing command-layer error mappings.

- [ ] **Step 3: Run variable RED tests**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_state_variable::tests::remove_variable_retains_next_revision_tombstone -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib commands::command_variable::tests::global_create_update_delete_history_restores_full_documents_and_publishes_once -- --exact --test-threads=1
```

Expected: the new removal test sees no revision entry; the history test still reports revision `3` as `missing`.

- [ ] **Step 4: Make variable validation document-independent**

Replace the `ResourceKey::Variable` branch in `validate_context_revisions` so it parses the stable variable ID and reads the ledger directly:

```rust
ResourceKey::Variable(path) => path
    .0
    .strip_prefix("variables/")
    .or(Some(path.0.as_ref()))
    .and_then(|id| uuid::Uuid::parse_str(id).ok())
    .map(crate::variable::VariableId::from)
    .and_then(|id| variable_revisions.get(&id).copied()),
```

Do not change the variable branch under `expected_absent_resources`.

- [ ] **Step 5: Advance rather than remove variable tombstones**

In `ResourceDocumentPatch::PatchVariables`, replace removal of the revision entry with monotonic advancement:

```rust
for id in removals {
    data.variables.remove(&id);
    let revision = variable_revisions
        .get(&id)
        .copied()
        .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL)
        .next();
    variable_revisions.insert(id, revision);
}
```

In `ProjectState::remove_variable`, replace `revisions.remove(variable_id)` with:

```rust
let revision = revisions
    .get(variable_id)
    .copied()
    .unwrap_or(crate::node_system::document::ResourceRevision::INITIAL)
    .next();
revisions.insert(*variable_id, revision);
```

- [ ] **Step 6: Run variable GREEN and focused suites**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_state_variable::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib commands::command_variable::tests -- --test-threads=1
```

Expected: all selected tests pass; create/update/delete/undo/redo revisions remain contiguous and stale tombstone requests are rejected.

- [ ] **Step 7: Run Task 1 gates, independent review, and progress publication**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Review only Task 1 changes for document-independent validation, monotonic advancement, no second authority, and unchanged absent-resource semantics. Resolve all Critical/Important findings. After fresh controller verification, append evidence to the ledger and immediately update Phase 4 in the `TODO.md` node architecture table.

### Task 2: Retain graph revision authority across both unload paths

**Files:**
- Modify: `src-tauri/src/project/project_state.rs:2871-2901, 3638-3699`
- Modify: `src-tauri/src/project/production_tests.rs:2298-2413`
- Modify: `src-tauri/src/project/project_activation.rs:tests module`
- Modify after review: `TODO.md` under `## node_architecture 进度`

**Interfaces:**
- Consumes: the graph revision inserted by load, create, and graph mutation paths.
- Produces: an unloaded graph absent from `ProjectData.graphs` but still present in `graph_revisions` at the exact unchanged revision.
- Preserves: compile invalidation, history clearing on unload, scoped-variable cleanup, lifecycle guard ownership, and disk state.

- [ ] **Step 1: Add exact unload retention assertions before production changes**

Extend `unloaded_caller_delta_revision_and_history_follow_graph_move` immediately after unloading the caller:

```rust
assert!(!state.get_data().unwrap().graphs.contains_key(&caller));
assert_eq!(
    state.revision_state_for_test().0.get(&caller),
    Some(&GraphRevision::new(1))
);
```

The existing rename/undo/redo assertions already prove revisions continue from `1 → 2 → 3 → 4` while the caller remains unloaded.

Add a lifecycle-specific test that creates and persists a graph, loads it with lifecycle token `1`, mutates it to revision `1`, unloads with token `2`, and asserts the graph is absent from `ProjectData.graphs` while `revision_state_for_test().0` still contains revision `1`.

- [ ] **Step 2: Add activation replacement coverage**

Add a project activation test that activates project A, creates a graph, unloads it to leave a revision entry, activates project B, and compares `revision_state_for_test()` with project B's prepared revisions. Assert the old graph path is absent from the new session ledger:

```rust
let (graphs, variables, worksheets) = state.revision_state_for_test();
assert!(!graphs.contains_key(&old_graph));
assert!(variables.keys().all(|id| new_data.variables.contains_key(id)));
assert!(worksheets.keys().all(|id| new_data.worksheets.contains_key(id)));
```

This test freezes session-scoped rather than persistent tombstones.

- [ ] **Step 3: Run graph RED tests**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::unloaded_caller_delta_revision_and_history_follow_graph_move -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_activation::tests::activation_replaces_old_session_revision_tombstones -- --exact --test-threads=1
```

Expected: the unloaded caller test fails because its revision entry is missing. The activation test may pass before the fix only if it reaches activation after establishing the tombstone; if so, retain it as a session-boundary regression and use the unload test as the required RED proof.

- [ ] **Step 4: Preserve graph revisions in both unload paths**

In both `unload_graph_resource` and `unload_graph_resource_for_lifecycle`, delete only this line:

```rust
self.graph_revisions.write().unwrap().remove(graph_path);
```

Do not insert or increment a revision. The currently recorded revision remains unchanged because unload is not a graph document mutation.

- [ ] **Step 5: Run graph GREEN and focused lifecycle/history suites**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::unloaded_caller_delta_revision_and_history_follow_graph_move -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_activation::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_lifecycle::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::resource_mutations::tests -- --test-threads=1
```

Expected: all selected tests pass; unloaded caller revisions are contiguous through rename/undo/redo; activation does not leak old-session entries.

- [ ] **Step 6: Run final gates and independent review**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Review Task 2 and the combined slice for exact unload semantics, lifecycle lock behavior, session reset, unchanged absent-resource checks, and interactions with Task 1. Resolve all Critical/Important findings and rerun their covering tests.

- [ ] **Step 7: Publish completion**

Append exact test counts, review verdicts, gates, and contracts to the ledger. Immediately update the relevant Phase 2 and Phase 4 rows in `TODO.md`. Record that no complete Rust suite was rerun and preserve all unrelated table content.
