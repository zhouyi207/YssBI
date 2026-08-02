# Resource Revision Tombstones Design

## Goal

Preserve optimistic-concurrency and undo/redo authority when a variable document is deleted or a graph document is unloaded from memory.

## Root cause

The current revision maps serve as resource revision ledgers, but some call sites treat them as caches of currently materialized documents:

- `validate_context_revisions` reads a variable revision only when the variable still exists in `ProjectData`.
- history `PatchVariables` removal deletes the variable revision entry.
- direct and lifecycle graph unload delete the graph revision entry even though the graph still exists on disk.

As a result, later history or cross-resource mutations compare an expected revision against `missing` rather than the last authoritative revision.

## Chosen design

Keep the existing `variable_revisions` and `graph_revisions` maps as the sole revision authority. A map entry may outlive the in-memory document and therefore acts as a session-scoped tombstone.

No new enum, map, serialization format, compatibility path, or persistence layer is introduced.

## Variable semantics

- Revision validation for `ResourceKey::Variable` resolves the variable ID and reads `variable_revisions` directly, regardless of whether `ProjectData.variables` contains the document.
- A normal global-variable deletion already advances and retains its revision; this behavior remains authoritative.
- A history `PatchVariables` removal deletes the document and advances the existing revision with `next()`, retaining the result as the tombstone.
- A history update or restore inserts the document and advances from the same ledger entry.
- A truly unknown variable without a revision entry remains `missing`.
- `expected_absent_resources` continues to inspect document presence only, so a tombstone does not make a deleted variable appear present.

This supports create → update → delete → undo-all → redo-all with one monotonically increasing revision sequence.

## Graph unload semantics

- Unloading removes the graph document from `ProjectData.graphs` and invalidates its compile products, but does not change graph content.
- Both `unload_graph_resource` and `unload_graph_resource_for_lifecycle` retain the existing `graph_revisions` entry unchanged.
- `ResourceDocumentPatch::UnloadGraph` already has this behavior and remains unchanged.
- Later cross-resource mutations may validate and advance an unloaded graph from the retained revision while keeping it unloaded.
- `expected_absent_resources` continues to inspect loaded/document presence according to its existing contract and is not changed by the retained revision entry.

## Lifetime and persistence

Tombstones are scoped to the active project session. Project activation reconstructs revision maps from the newly activated authoritative documents and clears the previous session's ledger. No tombstone is written to project files, because current history authority is also session-scoped.

## Locking and publication

All revision changes remain inside existing mutation publication critical sections and existing lock order. The design does not add I/O, waits, or new locks. Document removal and revision advancement remain atomic from the perspective of transaction validation and publication.

## Error behavior

- Correct current revisions validate successfully even when the document is deleted or unloaded.
- Stale revisions preserve the existing layer-specific contract: domain History APIs return typed `MutationConflict::StaleRevision`, while command boundaries retain their existing mapped codes. Tombstone tests assert the exact expected/current revisions rather than changing global error mapping.
- Unknown resource IDs without any ledger entry continue to report `missing`.
- Tombstones do not authorize creation paths that require `expected_absent_resources`; those checks remain based on document/resource presence.

## Testing

The two existing regressions provide the required RED/GREEN proof:

- `commands::command_variable::tests::global_create_update_delete_history_restores_full_documents_and_publishes_once`
- `project::production_tests::unloaded_caller_delta_revision_and_history_follow_graph_move`

Additional focused tests must freeze:

- stale variable revisions remain rejected while a tombstone exists;
- history removal increments rather than drops the variable revision;
- graph unload retains the exact revision without incrementing it;
- unloaded graph cross-resource updates preserve unloaded state while emitting contiguous deltas;
- project activation rebuilds revision authority without leaking old-session tombstones.

Verification uses focused serial Rust tests, `pnpm rust:check`, `pnpm rust:fmt:check`, and `git diff --check`. The known-red complete Rust suite is not rerun. After independent review, update the relevant `TODO.md` node architecture progress rows and record evidence in a dedicated SDD ledger.
