# Project Filesystem Transaction Architecture

## Status

Approved design for completing the project lifecycle and revisioned mutation architecture without compatibility paths or dual filesystem writers.

## Problem

Project document operations currently coordinate authoritative in-memory state, filesystem state, project replacement, graph lifecycle tokens, and runtime admission through several independent locks and workflows. Focused concurrency reviews exposed recurring failures:

- delayed project-index reads could cross project replacement;
- graph load, unload, and rename were not consistently scoped to a project instance;
- rename could race normal saves and restore stale filesystem snapshots;
- project activation could deadlock with pre-run function loading;
- create, duplicate, remove, save-as, and registered-project deletion used different filesystem ownership rules;
- full `ProjectData` snapshots could overwrite unrelated mutations committed while filesystem I/O was in progress.

Local lock additions cannot establish correctness because the missing boundary is architectural: all project-document filesystem operations need one transaction protocol.

## Goals

- Make `ProjectState.project_data` the authoritative project, graph, function, variable, and worksheet state.
- Give every project-document filesystem operation an explicit project-session owner.
- Serialize conflicting filesystem operations by normalized project root without holding state locks during I/O.
- Publish project activation atomically.
- Apply narrow authoritative patches instead of replacing state with stale full snapshots.
- Make stale project operations produce zero state, filesystem, event, or frontend effects.
- Preserve ordered revisioned results and project-instance correlation.
- Remove direct project-document writers after migration; do not retain compatibility paths.

## Non-goals

- This slice does not implement the localized Catalog or enable node creation UI.
- This slice does not redesign DuckDB table transactions, which retain their own database transaction model.
- This slice does not run full Rust tests until the final Task 7 checkpoint.

## Core types

```rust
pub struct ProjectSession {
    pub instance_id: ProjectInstanceId,
    pub root: NormalizedProjectRoot,
}

pub struct ProjectTransactionContext {
    pub session: ProjectSession,
    pub operation_id: OperationId,
    pub affected_resources: Vec<ResourceKey>,
}

pub struct ProjectFilesystemCoordinator {
    root_leases: RootLeaseRegistry,
}
```

`NormalizedProjectRoot` is the canonical lease identity. Equivalent path spellings must resolve to the same key, including roots that do not yet exist.

## Lease model

### Single-root operations

All reads or writes that inspect or mutate project-document layout acquire the coordinator lease for the normalized root.

Examples:

- project index and resource index reads;
- graph/function/worksheet loading;
- project flush and resource save;
- resource create, duplicate, remove, and rename;
- global-variable persistence;
- worksheet/reference cascades;
- active or registered project deletion.

### Multi-root operations

Save-as, project copy, and cross-project import:

1. normalize every root;
2. sort and deduplicate roots;
3. acquire leases in that deterministic order;
4. revalidate source ownership and destination policy while leased;
5. perform I/O;
6. release leases in reverse order.

This prevents source/destination lock-order deadlocks and destination TOCTOU overwrites.

### State lock exclusion

No operation may hold these locks while waiting for a filesystem lease or performing filesystem I/O:

- `mutation_publication`;
- `project_path`;
- `project_data`;
- `graph_lifecycle`;
- history or runtime-store locks.

The filesystem coordinator is a dedicated path-scoped serialization mechanism, not an authority store.

## Transaction protocol

### Capture

Before waiting for leases, capture:

- `ProjectInstanceId`;
- normalized project root;
- affected resource revisions;
- required immutable payload snapshots.

### Revalidation

After acquiring leases, revalidate project identity, project path, and resource revisions. Lifecycle cancellation takes precedence over filesystem errors from an obsolete session.

### Prepare

Write new content into a transaction staging directory:

```text
<project>/.yssbi-transaction/<operation-id>/
```

Validate all serialized documents before changing live files.

### Filesystem commit

Record precise before-images for only the target files and directory topology affected by the operation. Replace targets atomically where supported. Rollback restores only this transaction's mutation set.

The shared root lease ensures rollback cannot overwrite a concurrent project-document writer.

### Authoritative commit

After filesystem work, release all state locks if any were used for snapshots, then acquire the short publication boundary in the established order. Revalidate ownership and revisions again and apply a narrow `ResourceDocumentPatch` to current `ProjectData`.

A transaction must never commit a full `ProjectData` clone captured before I/O.

### Result publication

A successful authoritative commit returns and emits one correlated revisioned result containing:

- `projectInstanceId`;
- publication revision;
- resource deltas;
- exact projection membership or invalidations;
- backend history status.

Frontend consumers reject project-instance or publication-order mismatches before any side effect.

## Read transactions

Project-index and resource-load queries:

1. capture `ProjectSession`;
2. acquire the root lease without state locks;
3. perform disk reads;
4. reacquire the short publication/read boundary;
5. revalidate the session;
6. overlay loaded authoritative functions and globals from one coherent `ProjectData` snapshot;
7. return the result while still associated with the captured project identity.

Queries never mutate authoritative state from disk.

## Project activation

Activation separates preparation, drain, and publication:

1. acquire the normalized root lease;
2. read and prepare project/runtime data outside state locks;
3. release the root lease;
4. establish a run-admission drain guard for the currently published session;
5. wait for old pre-runs/runs without holding `mutation_publication` or a filesystem lease;
6. atomically publish the new identity, path, `ProjectData`, graph lifecycle registry, and runtime store;
7. release the drain guard.

Concurrent activations are serialized by a dedicated activation coordinator. No intermediate new-path/default-data state is visible.

## Graph lifecycle ownership

Graph operations use a strong owner:

```text
ProjectInstanceId + GraphPath + LifecycleToken + LifecycleIntent
```

The lifecycle registry is keyed by project instance and graph path. `Load`, `Unload`, and `Rename` are distinct intents.

- old-project tokens cannot match a replacement project;
- rename owns its resource exclusively through filesystem and authoritative commit;
- load returns a projection built from its operation-owned committed snapshot, not generic current state;
- unload and rename results/events carry `projectInstanceId` and are rejected by frontend consumers before any effect.

## Resource operations

### Save and flush

Snapshot current authoritative documents and revisions, acquire the root lease, revalidate freshness, persist, then report success. A stale snapshot never writes.

### Create and duplicate

Destination availability is checked only while holding the destination root lease. Filesystem and authoritative insertion belong to one transaction and use Rust-allocated persistent identities.

### Remove

Removal captures project/resource ownership, commits filesystem deletion under the root lease, and applies a narrow authoritative remove patch only if ownership and revision remain valid.

### Rename

Rename stages target documents and reference cascades, validates complete output, commits the precise filesystem mutation set, then applies a narrow resource-move patch to current `ProjectData`. It never replaces full project authority.

### Save-as and project copy

Acquire normalized source and destination leases in sorted order. Recheck destination emptiness under lease. Build the destination entirely from a coherent source snapshot and publish the new project only after successful filesystem completion.

### Registered-project deletion

Acquire the normalized root lease, invalidate/drain the active session if applicable, then move/delete the project directory. No index, load, save, rename, or worksheet operation may overlap the deletion.

## Error model

Structured failures distinguish:

- `stale_project_lifecycle`;
- `resource_revision_conflict`;
- `filesystem_transaction_busy`;
- `destination_not_empty`;
- `transaction_prepare_failed`;
- `transaction_commit_failed`;
- `transaction_rollback_failed`.

If filesystem commit succeeds but authoritative commit cannot complete, the error carries an explicit recovery requirement and triggers authoritative project reload. It is never reported as an ordinary uncommitted failure.

## Frontend contract

- Services send the current required `projectInstanceId`; it is never optional.
- Application coordinators capture a project epoch before invoke and reject stale completions.
- Events and direct results are gated by project identity before correlation or store access.
- Graph state changes only through authoritative projection replacement.
- Catalog-dependent creation remains explicitly disabled until the Catalog slice.
- The removed `get_editor_schema_command` is not restored. Active graph behavior uses projection capabilities; legacy schema/node-registry/global-type-system stores remain removed.

## Migration sequence

1. Introduce normalized root keys, deterministic lease acquisition, transaction contexts, and staging helpers.
2. Migrate project-index and resource-load reads.
3. Migrate project activation and run admission.
4. Migrate save/flush and global-variable persistence.
5. Migrate graph/function/worksheet create, duplicate, remove, and save.
6. Migrate rename and reference cascades to narrow authoritative patches.
7. Migrate save-as, project creation/copy, and registered-project deletion.
8. Remove old rollback snapshots and direct project-document writer entry points.
9. Add a source audit that fails when production project-document code bypasses the coordinator.
10. Resume the revisioned-mutation Task 7 verification checkpoint.

No compatibility layer or dual writer remains after each migration step.

## Testing strategy

Tasks before the final checkpoint use focused tests only with:

```text
CARGO_BUILD_JOBS=1
--test-threads=1
```

Required focused coverage includes:

- equivalent root paths share one lease;
- reverse-order multi-root operations do not deadlock;
- destination checks are repeated under lease;
- old project sessions have zero effects;
- activation and pre-run loading do not deadlock;
- graph load/unload/rename cannot cross project replacement;
- save/flush cannot interleave with rename rollback;
- narrow rename patches preserve unrelated concurrent mutations;
- staging prepare, commit, and rollback fault injection;
- worksheet topology and variable-file restoration;
- registered-project deletion excludes every reader/writer;
- every direct result and event is project-instance scoped;
- source audit finds no bypassing filesystem writer.

Only after focused frontend, Rust, formatting, type, and diff checks pass will Task 7 run the complete Rust suite exactly once:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- --test-threads=1
```

If that process exhausts memory or stalls, it is not retried; the exact termination is recorded.
