# Committed Resource Receipt Completion

## Status

Approved focused design for making every allocated resource publication revision observable exactly once.

## Invariant

Before `CommittedResourceMutation` exists, an operation may fail normally and allocate no publication revision. After it exists, completion is total: it must produce one canonical `ResourceMutationResultDto` and may not return an error.

## Pre-commit work

All fallible conditions that can prevent commit run before publication revision allocation:

- project lifecycle and recovery-required gate;
- request and resource revision validation;
- filesystem staging, commit, and rollback;
- history-head validation;
- payload normalization and cache preparation;
- projection membership and affected graph discovery.

A failure in this phase returns a normal structured error and creates no committed receipt.

## Commit boundary

The short authority publication boundary performs:

1. final lifecycle, revision, and history-head validation;
2. narrow `ProjectData` mutation;
3. Rust history mutation;
4. resource publication revision allocation;
5. coherent receipt capture.

```rust
pub struct CommittedResourceMutation {
    pub project_instance_id: ProjectInstanceId,
    pub publication_revision: u64,
    pub deltas: Vec<ResourceDeltaEvent>,
    pub moves: Vec<ResourceMoveDto>,
    pub history: HistoryStatusDto,
    pub projection_source: ProjectionSourceSnapshot,
    pub expected_graph_paths: Vec<String>,
}
```

The exact field visibility may remain internal, but the receipt owns every input required for completion. No live project/history/publication state is needed afterward.

## Total completion

```rust
impl CommittedResourceMutation {
    pub fn complete(self, locale: &str) -> ResourceMutationResultDto;
}
```

Completion:

- does not call `ensure_mutation_operational`;
- does not read live `ProjectState`;
- does not acquire project, history, publication, registry, or lifecycle locks;
- does not perform filesystem I/O;
- does not return `Result`;
- uses only receipt-owned immutable data.

If projection construction succeeds, the result carries complete replacements and exact expected graph paths.

If projection construction fails, completion logs the internal error and returns:

```text
projectionStatus = incomplete
projectionReplacements = []
invalidatedGraphPaths = expectedGraphPaths
```

A projection failure never erases or disguises an authoritative commit.

## Command and observer contract

Every resource mutation command follows:

```rust
let receipt = state.commit_operation(...)?;
let result = receipt.complete(&locale);
observer(&result);
Ok(result)
```

After receipt creation, command/application code contains no fallible `?` path before result return.

The observer and direct response receive the same DTO. The observer is invoked at most once. Public delta-only wrappers, split publication fields, and result reconstruction are prohibited.

Variable effects expose the complete resource mutation result through `RunResult`; Tauri does not rebuild it.

## Recovery-required race

If another operation marks the project recovery-required after this receipt commits but before completion:

- this receipt still completes and is published;
- subsequent new mutations fail the recovery gate;
- the frontend uses the observable publication plus recovery state to perform authoritative resynchronization.

The committed operation is never retroactively converted into an uncommitted error.

## Projection sources

`ProjectionSourceSnapshot` is captured after authoritative mutation inside the commit workflow and is read-only. This is permitted by the approved filesystem design; only pre-I/O full-state transaction authority is forbidden.

Projection source preparation itself must be non-fallible at completion time. Any registry/database metadata needed by projection building is captured in the receipt. External I/O or live state lookup is not deferred into `complete`.

## Tests

Focused tests must prove:

- signature commit remains observable when recovery-required is marked before completion;
- undo and redo have the same guarantee;
- injected projection failure returns an incomplete result rather than an error;
- observer and direct result serialize identically;
- the next mutation is rejected by the recovery gate;
- resource publication revisions remain contiguous and match ProjectIndex;
- source audit finds no `ensure_mutation_operational`, live-state read, filesystem I/O, or error-returning completion after receipt creation;
- no public delta-only or split publication wrapper remains.

Only focused `--lib` Rust tests are permitted. The complete Rust suite remains reserved for the final filesystem transaction checkpoint.

## Integration gate

This design is a prerequisite repair for Task 1 of `2026-07-27-project-publication-recovery.md`. After implementation and clean review:

1. mark the prior five-round Task 1 blocker as superseded by this approved focused repair;
2. complete publication recovery Task 1;
3. continue publication recovery Tasks 2–3;
4. reopen the main filesystem Task 3 only after the whole main Task 2 re-review is clean.
