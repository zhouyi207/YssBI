# History Survives Graph Cache Unload Design

## Status

Approved design for closing the remaining Phase 4 History lifecycle gap in
`docs/plan/node-architecture.md`.

This design preserves project-scoped Rust History when a graph document leaves
the in-memory graph cache. It does not persist History across project reloads or
application restarts.

## Problem

`ProjectState::unload_graph_resource` and
`ProjectState::unload_graph_resource_for_lifecycle` remove the graph and its
local variables from `ProjectState.project_data`, then clear the entire
`ProjectHistory`.

Graph unload is a cache/lifecycle operation, not a project document mutation.
Clearing History at this boundary has three incorrect consequences:

1. closing one graph discards Undo/Redo for unrelated project resources;
2. a cache residency decision changes the project transaction timeline;
3. History behavior differs between loaded and unloaded resources even though
   both represent the same persisted project documents.

Project reload and project activation replacement are different boundaries.
They replace the complete project authority and must continue to clear History.

## Goals

- Preserve the complete project History across direct and lifecycle graph
  unload.
- Keep an unloaded graph unloaded after Undo/Redo.
- Apply a History transaction atomically when it spans loaded and unloaded
  resources.
- Reuse the existing `ProjectHistory`, resource patch, filesystem transaction,
  revision, publication, and recovery machinery.
- Keep graph revisions monotonic across Undo/Redo.
- Keep Rust as the only History and project document authority.
- Perform filesystem I/O without holding global project locks.

## Non-goals

- Persisting History across project reloads or application restarts.
- History rebase, arbitrary timeline navigation, or collaborative editing.
- Reopening a graph or editor tab as a side effect of Undo/Redo.
- Introducing a History shadow document store.
- Reimplementing History in the frontend.
- Extending database History behavior in this slice.
- Implementing relational Filter/Project lineage, scheduler parallelism,
  execution deadlines, or runtime result memoization.

## Chosen approach

Use History-head-driven, on-demand hydration.

Before applying Undo/Redo, the backend inspects the current History head and
builds one temporary `ProjectDocumentState` containing every touched resource.
Loaded resources come from a short `ProjectState` snapshot. Missing graph,
function, and graph-local variable documents are loaded from the canonical
project filesystem outside global locks.

The existing `ProjectHistory::undo` or `ProjectHistory::redo` applies the
transaction to this temporary state. If the transaction contains an unloaded
resource, all affected durable documents are staged in one existing project
filesystem transaction. Commit-time validation then confirms that project
identity, History head, revisions, authority generation, and graph residency
still match the preparation snapshot.

A successful commit updates loaded resources in memory, persists unloaded
resources without loading them into `project_data`, advances revisions and
publication atomically, moves the History head, and emits the canonical
resource result. A failed validation or filesystem operation leaves all
project authority unchanged.

## Rejected alternatives

### Complete before/after snapshots in every History transaction

This would duplicate forward/inverse patches, increase History memory, change
the History serialization contract, and require migration across every
transaction constructor.

### Unloaded History shadow store

A shadow store would create a third document source beside `project_data` and
the filesystem. Its revisions and residency could diverge from both existing
authorities.

### Keep every graph document in `project_data`

This avoids hydration but makes unload a projection-only operation and prevents
actual document-cache eviction. It also preserves graph-local state in memory
indefinitely.

### Drop only transactions that touch the unloaded graph

Project History is a linear stack of atomic cross-resource transactions.
Selective removal can invalidate later patches and cannot safely split one
transaction into retained and discarded resources.

## Authority and residency model

Residency is preparation metadata, not a new persisted identity:

```rust
enum HistoryResourceResidency {
    Loaded,
    Unloaded {
        graph_path: GraphResourcePath,
    },
}
```

The concrete implementation may use a map or a more focused internal type, but
it must preserve these semantics:

- Stable `ResourceKey` remains the transaction identity.
- A graph path remains opaque and canonical.
- A Function document is resolved with its owning Function graph.
- Event/Function local variables are resolved with their owning graph.
- Global variables remain project-level resources.
- Presence tombstones remain owned by the variable revision ledger.
- Residency metadata never crosses IPC and is never serialized into History.

## Preparation flow

1. Ensure the current project is operational.
2. Snapshot the current project session, authority generation, History head ID,
   relevant revisions, resource residency, and loaded documents under short
   locks.
3. Release locks.
4. Resolve the History head's complete touched-resource set.
5. Load missing graph documents and graph-local resources through canonical
   project filesystem readers.
6. Build a temporary `ProjectDocumentState` containing every resource required
   by the transaction and its request anchor.
7. Validate the request base revision against the hydrated current state.
8. Apply the existing `ProjectHistory::undo` or `ProjectHistory::redo` to cloned
   History and document state.
9. If no unloaded resource is involved, retain the existing in-memory commit
   path.
10. If any unloaded graph is involved, stage all affected durable documents in
    one project filesystem transaction.
11. Commit the prepared filesystem transaction outside all project authority
    locks, keeping its rollback guard armed.
12. Reacquire commit locks in the established project mutation order.
13. Revalidate project instance, authority generation, History head ID,
    resource revisions, residency, and recovery state. A failed validation
    releases authority locks and rolls the committed filesystem mutation back.
14. Perform one precomputed, non-fallible authority swap for loaded documents,
    revision ledgers, History movement, compile invalidation, and publication.
15. Finalize the filesystem mutation only after the authority swap succeeds.
16. Complete projections and emit canonical deltas after finalization.

No project data, History, publication, or lifecycle lock may be held while
reading, staging, committing, finalizing, or rolling back filesystem content.
All fallible replay, serialization, delta construction, and projection-basis
construction must finish before the first live authority assignment.

## Persistence and write-back rules

| Resource state | Memory write-back | Filesystem write-back | Projection |
|---|---|---|---|
| Loaded Graph | yes | existing save semantics, or durable transaction when mixed with unloaded resources | yes |
| Unloaded Graph | no | yes | no |
| Loaded Function | with owning graph | same as owning graph | yes |
| Unloaded Function | no | with owning graph | no |
| Local variable | with owning graph | with owning graph | only when owning graph is loaded |
| Global variable | yes | existing project persistence path | existing canonical event behavior |

An ordinary loaded-only `InMemoryUntilSave` transaction remains in memory and
must not become durable merely because History is used. Existing
`DurableVariableEffects` and `DurableResourceMove` transactions continue to
use their established specialized durable workflows regardless of residency;
the new hydration dispatch must not intercept or duplicate those engines.

Only an `InMemoryUntilSave` transaction that touches an unloaded graph enters
the new durable hydration path. Every loaded and unloaded document affected by
that same cross-resource transaction—including project-level global variable
state—must be staged and committed together so that readers observe either the
complete before-state or complete after-state.

After a durable History commit, a later save must observe the newly committed
state and must not overwrite it with a stale loaded snapshot.

## Unload semantics

Both unload entry points must have identical History behavior:

- remove the graph document from the in-memory graph cache;
- remove its graph-local variable projection from in-memory project data;
- preserve graph and variable revision ledgers needed to address persisted
  resources;
- preserve both History stacks and the exact current head;
- invalidate graph or project compile products using the current rules;
- advance cache/publication authority only when residency actually changes;
- create no History transaction and emit no document mutation delta.

A stale lifecycle token or stale project instance changes neither residency nor
History.

## Undo/Redo behavior for unloaded resources

- Undo/Redo may target a History head whose request anchor or other touched
  resource is unloaded.
- Current revisions are obtained from coherent revision ledgers and hydrated
  documents, never inferred from insertion order or labels.
- Patch application uses the existing `ProjectHistory` engine.
- Revisions advance to new monotonic values; historical revision values are not
  restored.
- Unloaded resources remain absent from `project_data` after success.
- Loaded touched graphs remain loaded and receive projection replacements.
- Unloaded touched graphs receive deltas but no projection replacement.
- Cross-resource transactions never partially advance History.

## Concurrency and validation

Preparation records enough information to reject stale work at commit:

- project instance ID;
- project filesystem root/session;
- History head ID and direction;
- authority generation;
- expected revision keyed by every touched stable `ResourceKey`, including
  Function revisions embedded in owning graph documents and variable tombstone
  revisions; no separate Function revision ledger exists or may be added;
- loaded/unloaded residency of each touched graph;
- lifecycle ownership facts needed by the filesystem coordinator;
- a project filesystem lease held from hydration through staging, disk commit,
  authority validation, and finalization so unloaded Function revisions cannot
  change outside the coordinated transaction.

Commit fails without retry if any recorded fact changes, including:

- project activation or reload;
- a new mutation changing the History head;
- a touched revision changing;
- a graph being loaded or unloaded during preparation;
- recovery-required becoming active.

When a local variable is already unloaded, owner discovery uses the current
History patch's present-side scoped value and verifies it against the hydrated
graph document. It must not scan arbitrary graph files or treat History as a
shadow document authority.

The failure must use existing structured History, stale lifecycle, filesystem,
or recovery errors rather than a generic string where a stable code exists.

## Filesystem failure and recovery

Hydration failures such as missing files, invalid documents, path escape,
revision mismatch, or local-resource corruption occur before authority commit.
They leave History, memory, revisions, publication, and disk unchanged.

Durable History uses the existing filesystem journal and rollback behavior:

- staging or validation failure: zero authority effects;
- live replacement failure with successful rollback: zero committed authority
  effects;
- rollback failure: enter the existing recovery-required gate;
- no resource delta, History status, completion, or projection is published
  before a successful authority commit.

## Testing strategy

All behavior changes follow RED-GREEN TDD.

### History preservation

- A graph mutation creates an undoable transaction.
- Unloading that graph preserves `can_undo`, stack length, and History head.
- Unloading one graph preserves unrelated Graph/global-variable History.
- Direct unload and lifecycle unload have identical behavior.
- Stale lifecycle unload has zero History effect.

### Unloaded graph Undo/Redo

- Modify and persist a graph, unload it, Undo it from disk, and Redo it.
- Verify disk content after each direction.
- Verify the graph remains absent from `project_data`.
- Verify revisions strictly increase in both directions.
- Verify no projection replacement is produced for the unloaded graph.

### Mixed residency transaction

- Create one transaction touching loaded Graph A and unloaded Graph B.
- Undo and Redo atomically update both resources.
- Only A receives a projection replacement.
- Injected failure proves both remain at the before-state.

### Function and local variable coherence

- Undo/Redo a Function signature or local-variable change after unloading its
  Function graph.
- Verify ABI, scope, presence tombstone, revision ledger, and graph file remain
  coherent.
- Verify no standalone Function or local-variable authority is created.

### Reload boundary

- Project reload and replacement activation still clear History.
- History from one project cannot apply to another project instance.

### Failure and race coverage

- hydration read failure;
- staged serialization failure;
- first live replacement failure and rollback;
- rollback restoration failure entering recovery-required;
- History head changes after preparation;
- graph residency changes after preparation;
- authority generation or project instance changes after preparation.

Fixtures must use bounded waits and panic-safe temporary directories.

## Verification

Run focused serial Rust tests for:

- project History and document patches;
- direct and lifecycle graph unload;
- unloaded graph move/variable behavior;
- project filesystem transaction and recovery;
- command History publication where affected.

Then run:

```sh
pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Run broader project verification when the slice spans enough project/filesystem
boundaries to justify it. Do not claim completion without fresh controller
output and an independent final review.

## Completion criteria

- Neither unload path clears project History.
- Undo/Redo for an unloaded graph uses canonical disk hydration and the existing
  Rust History engine.
- Mixed loaded/unloaded transactions commit atomically.
- Unloaded graphs remain unloaded after Undo/Redo.
- Project reload remains the explicit History-reset boundary.
- No second document or History authority is introduced.
- Focused verification passes with no new Critical or Important review finding.
- `TODO.md` Phase 4 advances from 99% to 100% only after final slice review and
  fresh verification.
