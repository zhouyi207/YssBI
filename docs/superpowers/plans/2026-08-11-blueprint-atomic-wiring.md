# Blueprint Atomic Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver approved-spec Phase 1 atomic graph mutations, replacement and connection movement semantics, Rust-authored connection capabilities, and Blueprint-style frontend connection gestures without frontend-authored topology patches.

**Architecture:** Rust remains authoritative for graph topology, identity allocation, validation, patch ordering, revision, history, and final projection. React keeps only mutually exclusive interaction sessions and local pointer previews; pointer release sends one intent through the existing mutation coordinator, and committed projection replacement remains the only topology installation path.

**Tech Stack:** Rust, Serde, Tauri 2, TypeScript 5.8, React 19, Zustand 5, Vitest 4, pnpm 11.

## Global Constraints

- The approved source is `docs/superpowers/specs/2026-08-11-blueprint-style-graph-interaction-design.md`, especially sections 3–10, 15, 17, and Phase 1 in section 16.
- One user-visible graph operation produces one `GraphDocumentPatch`, one graph revision increment, and one Rust history entry.
- The frontend sends intent only; it never allocates graph identities, discovers authoritative affected connections, emits graph patches, or optimistically changes committed topology.
- Empty collection mutations fail; duplicate direct targets fail; derived shared resources are deduplicated; generated operations use deterministic identity order.
- `ConnectionsPerPort::Single` replacement is atomic. Full bounded `Multiple` ports remain non-replaceable.
- Validation failure, stale revision, invalid drop, and Escape cancellation preserve topology, revision, history, installed projection, and selection.
- Pointer movement, snapping, hover, and preview perform no IPC and remain animation-frame throttled.
- Tauri commands remain thin; frontend IPC remains under `src/services/`; global listeners continue through `src/shared/utils/globalEvent.ts`.
- Preserve every unrelated working-tree change observed before implementation; do not stage, rewrite, or revert it.
- Run repository commands from the repository root through `pnpm`; do not invoke Cargo from `src-tauri`.
- Only commit when the user explicitly requests a commit. Every task ends with a review checkpoint instead of a commit step.
- Do not retain compatibility shims for `deleteNode` or `disconnect`; this 0.x project replaces those wire variants directly.

## File Responsibility Map

- `src-tauri/src/node_system/document/mutation.rs`: wire mutation enum, stable mutation conflicts, atomic mutation planners, deterministic patch construction.
- `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs`: planner-level regression matrix for delete, disconnect, connect replacement, and movement.
- `src-tauri/src/node_system/analysis/projection.rs`: Rust-authored port connection capability projection.
- `src-tauri/src/node_system/compatibility.rs`: contextual create-and-connect source admission using append-or-replace capability.
- `src-tauri/src/commands/command_node_system.rs`: stable Tauri `AppError` mapping.
- `src-tauri/src/project/production_tests.rs`: revision, history, projection, stale, undo/redo, and concurrency acceptance coverage.
- `src/shared/types/dto/editorMutation.ts`: TypeScript mutation wire union.
- `src/shared/types/dto/editorProjection.ts`: TypeScript projection capability contract.
- `src/shared/types/dto/editorProjectionGuards.ts` and `src/shared/types/dto/editorMutationWireParser.ts`: exact projection boundary validation.
- `src/features/core/history/commands/`: one-intent command adapters.
- `src/features/core/canvas/canvasInteractionState.ts`: mutually exclusive canvas interaction sessions.
- `src/features/core/canvas/connectionTargeting.ts`: pure target compatibility, proximity, snapping, and advisory displacement calculation.
- `src/features/core/canvas/canvasPointerLoop.ts`: animation-frame preview updates and one pointer-release command.
- `src/views/EditorView/Canvas/core/ConnectionLine.tsx`, `EdgesOverlay.tsx`, and `src/views/EditorView/Pins/Pin.tsx`: preview and feedback rendering only.

---

### Task 1: Stable Phase 1 Mutation Conflict Codes

**Files:**
- Modify: `src-tauri/src/node_system/document/mutation.rs:90-186`
- Modify: `src-tauri/src/commands/command_node_system.rs:34-57`
- Test: `src-tauri/src/commands/command_node_system.rs` inline `mod tests`

**Interfaces:**
- Consumes: existing `MutationConflict`, `AppError`, and revision-specific command error codes.
- Produces: `EditorMutationConflictCode`, `MutationConflict::EditorRejected { code, message }`, `editor_rejected()`, and stable snake-case application error codes used by Tasks 2–4 and 10.

- [ ] **Step 1: Write the failing stable-code mapping test**

Add an inline command test with the complete Phase 1 code matrix:

```rust
#[test]
fn phase_one_editor_rejections_preserve_stable_app_error_codes() {
    use crate::node_system::document::{
        EditorMutationConflictCode as Code, MutationConflict,
    };

    for (code, expected) in [
        (Code::PortNotFound, "port_not_found"),
        (Code::OrphanPort, "orphan_port"),
        (Code::EndpointDirectionMismatch, "endpoint_direction_mismatch"),
        (Code::EndpointKindMismatch, "endpoint_kind_mismatch"),
        (Code::EndpointTypeMismatch, "endpoint_type_mismatch"),
        (Code::ConnectionLimitReached, "connection_limit_reached"),
        (Code::OrderedConnectionRequired, "ordered_connection_required"),
        (Code::ManagedNodeDeletionForbidden, "managed_node_deletion_forbidden"),
        (Code::ExistingConnection, "existing_connection"),
        (Code::EmptyMutationTargets, "empty_mutation_targets"),
        (Code::DuplicateMutationTarget, "duplicate_mutation_target"),
    ] {
        let error = mutation_conflict_to_app_error(
            MutationConflict::EditorRejected {
                code,
                message: "internal address detail".into(),
            },
            "graph_revision_conflict",
        );
        assert_eq!(error.code, expected);
    }
}
```

- [ ] **Step 2: Run the focused test and verify red**

Run: `pnpm rust:test -- phase_one_editor_rejections_preserve_stable_app_error_codes`

Expected: FAIL to compile because `EditorMutationConflictCode` and `EditorRejected` do not exist.

- [ ] **Step 3: Implement the stable conflict interface**

Add the enum and mapping in `mutation.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorMutationConflictCode {
    PortNotFound,
    OrphanPort,
    EndpointDirectionMismatch,
    EndpointKindMismatch,
    EndpointTypeMismatch,
    ConnectionLimitReached,
    OrderedConnectionRequired,
    ManagedNodeDeletionForbidden,
    ExistingConnection,
    EmptyMutationTargets,
    DuplicateMutationTarget,
}

impl EditorMutationConflictCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PortNotFound => "port_not_found",
            Self::OrphanPort => "orphan_port",
            Self::EndpointDirectionMismatch => "endpoint_direction_mismatch",
            Self::EndpointKindMismatch => "endpoint_kind_mismatch",
            Self::EndpointTypeMismatch => "endpoint_type_mismatch",
            Self::ConnectionLimitReached => "connection_limit_reached",
            Self::OrderedConnectionRequired => "ordered_connection_required",
            Self::ManagedNodeDeletionForbidden => "managed_node_deletion_forbidden",
            Self::ExistingConnection => "existing_connection",
            Self::EmptyMutationTargets => "empty_mutation_targets",
            Self::DuplicateMutationTarget => "duplicate_mutation_target",
        }
    }
}
```

Add `EditorRejected { code, message }` to `MutationConflict`, return `code.as_str()` from `MutationConflict::code()`, and display only its internal `message`. Add:

```rust
fn editor_rejected(
    code: EditorMutationConflictCode,
    message: impl Into<Box<str>>,
) -> MutationConflict {
    MutationConflict::EditorRejected {
        code,
        message: message.into(),
    }
}
```

Map the variant in `mutation_conflict_to_app_error()`:

```rust
crate::node_system::document::MutationConflict::EditorRejected { code, message } => {
    AppError::new(code.as_str(), message)
}
```

Keep `StaleRevision` mapped to the supplied revision conflict code.

- [ ] **Step 4: Run focused test and Rust check**

Run: `pnpm rust:test -- phase_one_editor_rejections_preserve_stable_app_error_codes`

Expected: PASS; one test runs and every returned `AppError.code` matches the matrix.

Run: `pnpm rust:check`

Expected: PASS with no Rust compiler errors.

- [ ] **Step 5: Review checkpoint**

Inspect `git diff -- src-tauri/src/node_system/document/mutation.rs src-tauri/src/commands/command_node_system.rs`. Confirm stable codes contain no UUID/address data, revision conflicts retain `graph_revision_conflict`, and no commit was created.

---

### Task 2: Atomic DeleteNodes and Disconnect Variants

**Files:**
- Modify: `src-tauri/src/node_system/document/mutation.rs:262-552,725-744,1490-1535`
- Modify: `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs:622-680`
- Modify: `src-tauri/src/commands/command_project/query.rs` test constructors using old variants
- Test: `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs`

**Interfaces:**
- Consumes: `editor_rejected()` and `EditorMutationConflictCode` from Task 1; `GraphDocumentOperation`; sortable `NodeId`, `ConnectionId`, and `PortAddress` identities.
- Produces: `EditorGraphMutationDto::{DeleteNodes, DisconnectConnections, DisconnectPort, DisconnectNode}`, `delete_editor_nodes_operations()`, and `disconnect_connection_operations()`.

- [ ] **Step 1: Replace singular test construction and add failing atomic deletion tests**

Use these wire variants in tests:

```rust
EditorGraphMutationDto::DeleteNodes { node_ids: vec![first, second] }
EditorGraphMutationDto::DisconnectConnections { connection_ids: vec![connection_id] }
EditorGraphMutationDto::DisconnectPort { address: declared(first, "data_out").into() }
EditorGraphMutationDto::DisconnectNode { node_id: first }
```

Add tests named:

```text
delete_nodes_removes_shared_resources_once_in_deterministic_order
delete_nodes_rejects_empty_duplicate_missing_and_managed_sets_atomically
disconnect_variants_derive_validate_and_sort_authoritative_connections
```

The first test creates two ordinary nodes, one internal connection, one external incident connection, one input state, and one dynamic binding, then compares the full operation vector in the required connection/state/binding/node order. The second clones the document before each empty, duplicate, missing, and managed-node request and compares the clone after every rejection. The third creates three connections sharing a port and node, then compares the explicit, port-derived, and node-derived removal vectors by exact connection identity.

The assertions must compare full `GraphDocumentOperation` vectors, including connection IDs and addresses, rather than operation counts alone.

- [ ] **Step 2: Run focused tests and verify red**

Run: `pnpm rust:test -- delete_nodes_`

Expected: FAIL to compile because `DeleteNodes` does not exist.

Run: `pnpm rust:test -- disconnect_variants_`

Expected: FAIL to compile because the new disconnect variants do not exist.

- [ ] **Step 3: Replace the Rust wire variants and dispatch arms**

Define:

```rust
DeleteNodes { node_ids: Vec<NodeId> },
DisconnectConnections { connection_ids: Vec<ConnectionId> },
DisconnectPort { address: PortAddressDto },
DisconnectNode { node_id: NodeId },
```

Delete `DeleteNode` and `Disconnect`. Dispatch each variant to one planner invocation. Reject empty direct collections and repeated direct identities before looking up resources.

- [ ] **Step 4: Implement deterministic delete planning**

Implement `delete_editor_nodes_operations()` with `BTreeSet`s. Validate every selected node and managed role before generating operations. Scan each document collection once, then append operations in this exact sequence:

```rust
connections.sort_by_key(|connection| connection.id);
input_states.sort_by(|left, right| left.0.cmp(&right.0));
port_bindings.sort_by(|left, right| left.0.cmp(&right.0));
nodes.sort_by_key(|node| node.id);
```

Map them to `RemoveConnection`, clearing `SetInputState`, `RemovePortBinding`, and `RemoveNode` respectively. Internal edges are selected by a set membership check and therefore appear once.

- [ ] **Step 5: Implement the shared disconnect planner**

Implement:

```rust
fn disconnect_connection_operations(
    document: &GraphDocument,
    connection_ids: Vec<ConnectionId>,
    reject_duplicate_direct_targets: bool,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict>
```

For explicit IDs, reject empty and duplicates. For port/node variants, validate the port/node first, derive IDs from the authoritative document, deduplicate through `BTreeSet`, reject an empty result, then resolve every connection before returning sorted removal operations.

- [ ] **Step 6: Run focused and existing deletion tests**

Run: `pnpm rust:test -- delete_nodes_`

Expected: PASS; empty/duplicate/missing/managed sets fail without a patch, and shared resources appear once in deterministic order.

Run: `pnpm rust:test -- disconnect_variants_`

Expected: PASS; all three disconnect paths return one complete sorted patch.

Run: `pnpm rust:test -- editor_delete_`

Expected: PASS after existing singular tests are updated to array length one.

Run: `pnpm rust:check`

Expected: PASS with no remaining Rust references to `EditorGraphMutationDto::DeleteNode` or `EditorGraphMutationDto::Disconnect`.

- [ ] **Step 7: Review checkpoint**

Run: `git --no-pager diff --check`

Expected: no whitespace errors. Review the mutation diff and confirm no compatibility variant, per-node loop, or per-connection loop exists at the wire-dispatch level. Do not commit.

---

### Task 3: Atomic Connect Replacement Planner

**Files:**
- Modify: `src-tauri/src/node_system/document/mutation.rs:434-479,1050-1124`
- Modify: `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs:740-848`
- Test: `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs`

**Interfaces:**
- Consumes: stable connection conflict codes from Task 1 and existing `resolve_mutation_port()`.
- Produces: `plan_connection_change()` returning validated removals plus one inserted `DocumentConnection`; reused by `Connect`, create-and-connect, and Task 4 movement.

- [ ] **Step 1: Add the failing replacement matrix**

Add uniquely named tests:

```rust
connect_replaces_occupied_single_input
connect_replaces_occupied_single_output
connect_replaces_two_independent_single_incumbents
connect_deduplicates_shared_incumbent
connect_rejects_existing_endpoint_pair_without_patch
connect_rejects_full_bounded_multiple_without_removal
connect_replacement_validation_failure_preserves_document
```

Each success test must assert sorted old `RemoveConnection` operations followed by one `InsertConnection`. Each failure test must clone the document, call `plan()`, assert the stable conflict code, and assert the clone still equals the original.

- [ ] **Step 2: Run the replacement filter and verify red**

Run: `pnpm rust:test -- connect_replaces_`

Expected: FAIL because occupied `Single` currently returns `connection limit` instead of replacement operations.

- [ ] **Step 3: Introduce a reusable connection plan result**

Add:

```rust
struct PlannedConnectionChange {
    removals: Vec<DocumentConnection>,
    insertion: DocumentConnection,
}

impl PlannedConnectionChange {
    fn into_operations(mut self) -> Vec<GraphDocumentOperation> {
        self.removals.sort_by_key(|connection| connection.id);
        let mut operations = self.removals
            .into_iter()
            .map(|connection| GraphDocumentOperation::RemoveConnection { connection })
            .collect::<Vec<_>>();
        operations.push(GraphDocumentOperation::InsertConnection {
            connection: self.insertion,
        });
        operations
    }
}
```

- [ ] **Step 4: Implement staged replacement validation**

In `plan_connection_change()`:

1. Resolve both ports and reject orphan/direction/kind/order violations with stable codes.
2. Reject an existing exact output/input pair before allocating a new ID.
3. Collect incumbents only from occupied `Single` endpoints into `BTreeSet<ConnectionId>`.
4. Keep bounded `Multiple` capacity strict; calculate capacity after planned removals.
5. Build a staged `GraphDocument` clone, remove incumbents, insert the proposed connection, and run the same document validation path used by committed patches.
6. Return removals plus insertion only after all checks pass.

Call this helper from `connect_operations()`. Update create-and-connect so an occupied replaceable `Single` source reaches this planner instead of failing the old strict capacity precheck.

- [ ] **Step 5: Run replacement, capacity, direction, kind, orphan, and order tests**

Run: `pnpm rust:test -- connect_`

Expected: PASS for replacement tests; full bounded `Multiple`, duplicate pair, direction, kind, orphan, and invalid ordering remain rejected.

Run: `pnpm rust:test -- editor_mutation_enforces_connection_capacity_and_order_policy`

Expected: PASS with the test split between replaceable `Single` and non-replaceable bounded `Multiple` behavior.

Run: `pnpm rust:check`

Expected: PASS.

- [ ] **Step 6: Review checkpoint**

Inspect the patch ordering and staged validation path. Confirm connection IDs are allocated only after duplicate-pair rejection, shared incumbents are removed once, and authority state is not mutated by planning. Do not commit.

---

### Task 4: Atomic MoveConnections Mutation

**Files:**
- Modify: `src-tauri/src/node_system/document/mutation.rs:262-552,1050-1124`
- Modify: `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs`
- Test: `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs`

**Interfaces:**
- Consumes: `plan_connection_change()` from Task 3.
- Produces: `EditorGraphMutationDto::MoveConnections { source, target }` and `move_connections_operations()` preserving moved connection IDs and order keys.

- [ ] **Step 1: Add failing movement tests**

Add tests named:

```rust
move_connections_moves_all_output_edges_atomically
move_connections_moves_all_input_edges_atomically
move_connections_replaces_single_target_incumbent
move_connections_rejects_many_edges_into_single_target
move_connections_preserves_ids_and_order_keys
move_connections_failure_preserves_original_topology
move_connections_rejects_empty_source_same_address_direction_kind_type_and_orphan
```

Construct at least one source with two connections and one target whose second moved edge is invalid. Assert the complete plan fails and the source edges remain unchanged in the original document.

- [ ] **Step 2: Run focused movement tests and verify red**

Run: `pnpm rust:test -- move_connections_`

Expected: FAIL to compile because `MoveConnections` is absent.

- [ ] **Step 3: Add the wire variant and planner**

Define:

```rust
MoveConnections {
    source: PortAddressDto,
    target: PortAddressDto,
},
```

Implement `move_connections_operations(document, registry, source, target)` to:

- resolve non-orphan ports;
- reject equal addresses;
- require equal direction and kind;
- collect all authoritative source connections sorted by ID;
- reject an empty source;
- replace only the source-side endpoint on each connection;
- retain each moved connection ID;
- retain order when the unchanged input remains the ordered endpoint;
- validate the full final staged document before returning operations;
- include replaceable target incumbents once;
- reject multiple moved edges into a `Single` target.

Represent each moved edge as `RemoveConnection { old }` followed later by `InsertConnection { updated_with_same_id }`; sort all removals by ID before all insertions by ID so inverse patch ordering is deterministic.

- [ ] **Step 4: Run focused tests and Rust check**

Run: `pnpm rust:test -- move_connections_`

Expected: PASS; success moves all edges in one patch and failure leaves the source topology intact.

Run: `pnpm rust:check`

Expected: PASS.

- [ ] **Step 5: Review checkpoint**

Review the movement planner and verify its request contains no connection IDs, every connection is resolved from `document.connections`, and no operation is emitted until the complete staged topology validates. Do not commit.

---

### Task 5: Rust-Authored Append, Replace, and Move Capabilities

**Files:**
- Modify: `src-tauri/src/node_system/analysis/projection.rs:310-315,1081-1102,1525-1532`
- Modify: `src-tauri/src/node_system/compatibility.rs:48-61`
- Test: `src-tauri/src/node_system/analysis/projection.rs` inline tests
- Test: `src-tauri/src/node_system/compatibility.rs` inline tests

**Interfaces:**
- Consumes: `ConnectionsPerPort`, current authoritative connection count, and orphan status.
- Produces: `PortConnectionCapabilityDto { current, maximum, ordered, can_append, can_replace, can_move }` and append-or-replace catalog source admission.

- [ ] **Step 1: Add failing capability truth-table tests**

Add projection tests for empty `Single`, occupied `Single`, partially full bounded `Multiple`, full bounded `Multiple`, unbounded `Multiple`, and orphan. Assert exact structs. Add a compatibility test proving occupied replaceable `Single` is admitted as a contextual create-and-connect source.

- [ ] **Step 2: Run focused tests and verify red**

Run: `pnpm rust:test -- projects_connection_capabilities_`

Expected: FAIL because the DTO still exposes `can_connect`.

Run: `pnpm rust:test -- compatibility_accepts_replaceable_source`

Expected: FAIL because `source_from_projection()` rejects occupied `Single`.

- [ ] **Step 3: Replace the projection field**

Use:

```rust
pub struct PortConnectionCapabilityDto {
    pub current: u32,
    pub maximum: Option<u32>,
    pub ordered: bool,
    pub can_append: bool,
    pub can_replace: bool,
    pub can_move: bool,
}
```

Calculate:

```rust
let can_append = !orphan && maximum.is_none_or(|limit| current < limit);
let can_replace = !orphan
    && matches!(capability, ConnectionsPerPort::Single)
    && current == 1;
let can_move = !orphan && current > 0;
```

For invalid over-capacity loaded state, both append and replace must be false; existing compiler diagnostics remain unchanged.

- [ ] **Step 4: Update contextual compatibility admission**

Replace the old source gate with:

```rust
if port.orphan || !(port.connections.can_append || port.connections.can_replace) {
    return Err(format!("source port '{address}' cannot participate in a connection"));
}
```

- [ ] **Step 5: Run focused tests and Rust check**

Run: `pnpm rust:test -- projects_connection_capabilities_`

Expected: PASS for the complete truth table.

Run: `pnpm rust:test -- compatibility_accepts_replaceable_source`

Expected: PASS.

Run: `pnpm rust:check`

Expected: PASS with no `can_connect` field references in Rust.

- [ ] **Step 6: Review checkpoint**

Confirm full bounded `Multiple` has `can_replace == false`, occupied non-orphan `Single` has `can_replace == true`, and capability values remain advisory because mutation planners revalidate. Do not commit.

---

### Task 6: TypeScript Wire Contracts and Atomic Command Adapters

**Files:**
- Modify: `src/shared/types/dto/editorMutation.ts:25-54`
- Modify: `src/features/core/history/commands/deleteNodes.ts`
- Rename: `src/features/core/history/commands/disconnectPin.ts` to `src/features/core/history/commands/disconnectPort.ts`
- Create: `src/features/core/history/commands/disconnectNode.ts`
- Create: `src/features/core/history/commands/moveConnections.ts`
- Modify: `src/features/core/history/commands/index.ts`
- Modify: `src/features/core/history/commands/registryTypes.ts`
- Modify: `src/features/application/editor/useEditorOperations.ts:81-143`
- Modify: `src/services/nodeSystem/graphMutationService.test.ts`
- Modify: `src/features/application/editorMutation/editorMutationCoordinator.test.ts`
- Test: `src/features/core/history/editorCommands.test.ts`

**Interfaces:**
- Consumes: Rust wire names from Tasks 2 and 4, structured projected `Pin.address`, and `executeGraphIntent()`.
- Produces: TypeScript mutation variants and one-call commands `DeleteNodes`, `DisconnectPort`, `DisconnectNode`, and `MoveConnections`.

- [ ] **Step 1: Rewrite command tests to demand one intent**

Replace singular expectations with:

```ts
expect(executeEditorMutation).toHaveBeenCalledTimes(1);
expect(executeEditorMutation).toHaveBeenCalledWith({
  graphPath,
  locale: 'en-US',
  mutation: {
    type: 'deleteNodes',
    payload: { nodeIds: ['node-1', 'node-2', 'node-3'] },
  },
});
```

Add exact expectations for:

```ts
{ type: 'disconnectPort', payload: { address: fixture.inputAddress } }
{ type: 'disconnectNode', payload: { nodeId: 'local-node' } }
{ type: 'moveConnections', payload: { source: fixture.outputAddress, target: fixture.inputAddress } }
```

Assert no command mutates `useGraphDataStore` before the coordinator result.

- [ ] **Step 2: Run command tests and verify red**

Run: `pnpm test -- src/features/core/history/editorCommands.test.ts`

Expected: FAIL because delete/disconnect still sequence singular intents and movement/node disconnect commands are absent.

- [ ] **Step 3: Replace the TypeScript mutation union**

In `editorMutation.ts`, remove `deleteNode` and `disconnect` and add:

```ts
| { type: 'deleteNodes'; payload: { nodeIds: string[] } }
| { type: 'disconnectConnections'; payload: { connectionIds: string[] } }
| { type: 'disconnectPort'; payload: { address: PortAddressDto } }
| { type: 'disconnectNode'; payload: { nodeId: string } }
| {
    type: 'moveConnections';
    payload: { source: PortAddressDto; target: PortAddressDto };
  }
```

Update test fixtures in the service and coordinator to use `deleteNodes` with an array of length one.

- [ ] **Step 4: Implement one-intent command handlers**

`deleteNodes.ts` executes exactly:

```ts
return executeGraphIntent(graphPath, {
  type: 'deleteNodes',
  payload: { nodeIds: args.nodeIds },
});
```

`disconnectPort.ts` resolves one projected pin address and executes `disconnectPort`; it never reads `pinConnections`. `disconnectNode.ts` forwards the node ID. `moveConnections.ts` resolves source and target pin addresses and forwards them. Register all commands in `index.ts` and `registryTypes.ts`.

Replace `useEditorOperations.breakAllNodeLinks()` pin iteration with one `DisconnectNode` command call.

- [ ] **Step 5: Run focused frontend tests**

Run: `pnpm test -- src/features/core/history/editorCommands.test.ts`

Expected: PASS; every complex command calls `executeEditorMutation` once.

Run: `pnpm test -- src/features/application/editor/useEditorOperations.capabilities.test.tsx`

Expected: PASS; managed-node protection remains and ordinary node break-links routes once.

Run: `pnpm test -- src/services/nodeSystem/graphMutationService.test.ts src/features/application/editorMutation/editorMutationCoordinator.test.ts`

Expected: PASS with updated wire fixtures.

Run: `pnpm typecheck`

Expected: PASS with no TypeScript references to mutation types `deleteNode` or `disconnect`.

- [ ] **Step 6: Review checkpoint**

Search `src/` for the removed wire strings and inspect command adapters. Confirm frontend code sends addresses/node IDs only and performs no connection-ID discovery for port/node operations. Do not commit.

---

### Task 7: TypeScript Projection Capability and Structured Compatibility

**Files:**
- Modify: `src/shared/types/dto/editorProjection.ts:128-133`
- Modify: `src/shared/types/dto/editorProjectionGuards.ts:148-154`
- Modify: `src/shared/types/dto/editorMutationWireParser.ts:197-204`
- Modify: `src/shared/utils/pinCompatibility.ts:77-111`
- Modify: `src/tests/helpers/editorProjectionFixtures.ts`
- Test: `src/shared/types/dto/editorMutationWireParser.test.ts`
- Test: `src/features/domain/editorProjection/editorProjection.test.ts`
- Test: `src/features/core/dataStore/graphProjectionStore.test.ts`
- Test: `src/shared/utils/pinCompatibility.test.ts`

**Interfaces:**
- Consumes: Rust capability JSON from Task 5.
- Produces: `PortConnectionCapabilityDto`, `ConnectionCompatibility`, and `resolveConnectionCompatibility()` used by targeting and gestures.

- [ ] **Step 1: Add failing exact-guard and compatibility tests**

Require exact capability keys:

```ts
{
  current: 1,
  maximum: 1,
  ordered: false,
  canAppend: false,
  canReplace: true,
  canMove: true,
}
```

Add compatibility assertions for `append`, `replace`, and invalid reasons `samePort`, `sameNode`, `directionMismatch`, `kindMismatch`, `typeMismatch`, `orphan`, and `capacityReached`. Include effect/control pins with disabled Rust capability to prevent the current early `return true` bypass.

- [ ] **Step 2: Run focused tests and verify red**

Run: `pnpm test -- src/shared/types/dto/editorMutationWireParser.test.ts src/shared/utils/pinCompatibility.test.ts`

Expected: FAIL because guards require `canConnect` and compatibility returns only boolean.

- [ ] **Step 3: Replace the DTO and exact guards**

Define:

```ts
export interface PortConnectionCapabilityDto {
  current: number;
  maximum: number | null;
  ordered: boolean;
  canAppend: boolean;
  canReplace: boolean;
  canMove: boolean;
}
```

Both guards must require exactly:

```ts
['current', 'maximum', 'ordered', 'canAppend', 'canReplace', 'canMove']
```

Update all fixtures without deriving these fields in React stores.

- [ ] **Step 4: Implement structured compatibility**

Add:

```ts
export type ConnectionCompatibility =
  | { kind: 'append' }
  | { kind: 'replace' }
  | {
      kind: 'invalid';
      reason: 'samePort' | 'sameNode' | 'directionMismatch' |
        'kindMismatch' | 'typeMismatch' | 'orphan' | 'capacityReached';
    };
```

Implement `resolveConnectionCompatibility(a, b, typeSystem)` so capability checks occur before effect/control success. Return `replace` when either endpoint requires replacement and both endpoints otherwise validate; return `append` only when both can append. Keep `canConnectPins()` as `resolveConnectionCompatibility(...).kind !== 'invalid'` for existing callers.

- [ ] **Step 5: Run focused projection and compatibility tests**

Run: `pnpm test -- src/shared/types/dto/editorMutationWireParser.test.ts src/features/domain/editorProjection/editorProjection.test.ts src/features/core/dataStore/graphProjectionStore.test.ts src/shared/utils/pinCompatibility.test.ts`

Expected: PASS; old `canConnect` payloads are rejected and effect/control capability is enforced.

Run: `pnpm typecheck`

Expected: PASS with no `.canConnect` capability references.

- [ ] **Step 6: Review checkpoint**

Confirm React treats capabilities as advisory input and does not recompute replacement authority from `current/maximum`. Do not commit.

---

### Task 8: Mutually Exclusive Canvas Interaction State

**Files:**
- Create: `src/features/core/canvas/canvasInteractionState.ts`
- Create: `src/features/core/canvas/canvasInteractionState.test.ts`
- Modify: `src/features/core/gesture/useGestureStore.ts`
- Modify: `src/shared/types/ui/editor.ts:110-155`
- Modify: `src/features/core/canvas/selectionSession.ts`
- Modify: `src/features/core/canvas/connectPreview.ts`
- Modify: `src/features/core/canvas/dragPreview.ts`
- Modify: `src/features/core/canvas/useCanvasInteraction.ts`
- Modify: `src/features/core/canvas/canvasPointerLoop.ts`
- Modify: `src/features/application/editor/useCanvasOverlayHandlers.ts`
- Test: `src/features/core/canvas/canvasInteractionState.test.ts`
- Test: `src/features/core/canvas/canvasPointerLoop.test.ts`

**Interfaces:**
- Consumes: `ConnectionCompatibility` from Task 7 and existing pin/viewport/session types.
- Produces: `CanvasInteraction`, `useCanvasInteractionStore`, `cancelCanvasInteraction()`, and exact session selectors used by preview renderers and keyboard handling.

- [ ] **Step 1: Write failing state transition tests**

Test exact transitions:

```ts
idle -> drawingConnection -> idle
idle -> movingConnections -> idle
idle -> selecting -> idle
idle -> draggingNodes -> idle
idle -> pendingNodeCreation -> idle
```

Assert starting any session replaces the previous session, and cancellation returns the cancelled type. Include `panning` in the same union to preserve existing pan while maintaining exclusivity.

- [ ] **Step 2: Run the state test and verify red**

Run: `pnpm test -- src/features/core/canvas/canvasInteractionState.test.ts`

Expected: FAIL because the state module does not exist.

- [ ] **Step 3: Define the complete interaction union**

Create:

```ts
export type CanvasInteraction =
  | { type: 'idle' }
  | { type: 'panning'; session: PanSession }
  | { type: 'selecting'; session: SelectionSession }
  | { type: 'draggingNodes'; session: NodeDragSession }
  | { type: 'drawingConnection'; session: ConnectionDrawSession }
  | { type: 'movingConnections'; session: ConnectionMoveSession }
  | { type: 'pendingNodeCreation'; session: PendingNodeCreationSession };
```

Both connection sessions contain `groupId`, `graphPath`, `source`, pointer screen/world coordinates, `hoveredTarget`, `snappedTarget`, and `compatibility`. `ConnectionMoveSession` contains no connection IDs.

Create a Zustand store with `interaction`, `start(next)`, `updateConnectionPreview(update)`, and `cancel()`; `cancel()` returns the previous interaction type for Escape precedence tests.

- [ ] **Step 4: Migrate existing fragmented state**

Move selection activity, node drag, connect gesture, and pending node creation into the union. Keep node position overrides in `graphInteractionStore`; they are preview data, not interaction identity. Adapt `connectPreview.ts` and `dragPreview.ts` subscriptions to selectors over the new state. Remove the old nullable `connect` gesture once all consumers compile.

- [ ] **Step 5: Run state and pointer-loop tests**

Run: `pnpm test -- src/features/core/canvas/canvasInteractionState.test.ts src/features/core/canvas/canvasPointerLoop.test.ts`

Expected: PASS; only one session is active and existing node movement still emits one final `MoveNodes` command.

Run: `pnpm typecheck`

Expected: PASS with no old `gesture.type === 'connect'` branches.

- [ ] **Step 6: Review checkpoint**

Inspect the state graph and verify pan was preserved inside the exclusive union, pending node creation is not a second independent boolean, and connection movement stores intent rather than connection IDs. Do not commit.

---

### Task 9: Pure Connection Targeting, Snapping, and Feedback

**Files:**
- Create: `src/features/core/canvas/connectionTargeting.ts`
- Create: `src/features/core/canvas/connectionTargeting.test.ts`
- Modify: `src/features/core/canvas/canvasPointerLoop.ts:70-158,188-260`
- Modify: `src/features/core/canvas/connectPreview.ts`
- Modify: `src/views/EditorView/Canvas/core/ConnectionLine.tsx`
- Modify: `src/views/EditorView/Canvas/core/ConnectionLine.test.tsx`
- Modify: `src/views/EditorView/Canvas/core/EdgesOverlay.tsx`
- Modify: `src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx`
- Modify: `src/views/EditorView/Pins/Pin.tsx:317-370`
- Test: `src/features/core/canvas/connectionTargeting.test.ts`

**Interfaces:**
- Consumes: `resolveConnectionCompatibility()`, current graph projection, current canvas DOM geometry, and Task 8 interaction sessions.
- Produces: `resolveConnectionTarget()` and `ConnectionTargetResult` containing hovered target, snapped target, compatibility, and advisory displaced connection IDs.

- [ ] **Step 1: Add failing pure targeting tests**

Define test inputs as pin centers and projected pins, then assert:

- nearest compatible pin inside radius snaps;
- occupied replaceable `Single` returns `replace` and incumbent IDs;
- full bounded `Multiple` returns invalid and does not snap;
- invalid nearest pin does not hide a slightly farther valid pin inside radius;
- leaving the radius clears snapped target and displaced IDs.

- [ ] **Step 2: Run targeting test and verify red**

Run: `pnpm test -- src/features/core/canvas/connectionTargeting.test.ts`

Expected: FAIL because `resolveConnectionTarget()` is absent.

- [ ] **Step 3: Implement the pure targeting result**

Use:

```ts
export interface ConnectionTargetResult {
  hoveredTarget: Pin | null;
  snappedTarget: Pin | null;
  compatibility: ConnectionCompatibility | null;
  displacedConnectionIds: string[];
}
```

`resolveConnectionTarget()` sorts candidates by squared screen distance, checks only candidates inside the centralized snap radius, uses structured compatibility, and derives advisory displaced IDs from the installed projection only for `replace`. It never invokes a service or mutates a store.

- [ ] **Step 4: Integrate animation-frame geometry and preview rendering**

In the active canvas only, query `[data-pin-id]`, calculate element centers, and resolve projected pins on each existing throttled frame. Update the connection session once per frame. Draw the preview endpoint at the snapped target world position. Add data attributes/classes on `Pin.tsx` for append, replace, and invalid. Add advisory displaced-edge classes in `EdgesOverlay.tsx`; Rust remains authoritative for actual removals.

- [ ] **Step 5: Run targeting and rendering tests**

Run: `pnpm test -- src/features/core/canvas/connectionTargeting.test.ts src/views/EditorView/Canvas/core/ConnectionLine.test.tsx src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx`

Expected: PASS; snapping and feedback update locally and no test observes an IPC call during pointer movement.

Run: `pnpm typecheck`

Expected: PASS.

- [ ] **Step 6: Review checkpoint**

Confirm targeting is canvas-scoped, animation-frame throttled, valid and replacement targets snap, invalid targets submit nothing, and displaced-edge highlighting is advisory. Do not commit.

---

### Task 10: Ctrl-Drag, Alt-Disconnect, Escape Precedence, and Safe Errors

**Files:**
- Modify: `src/features/core/canvas/useCanvasInteraction.ts:124-146`
- Modify: `src/features/core/canvas/canvasPointerLoop.ts:188-260`
- Modify: `src/features/application/editor/useEditorKeyboard.ts:81-126`
- Modify: `src/features/application/editor/useEditorKeyboard.test.tsx`
- Create: `src/features/application/editorMutation/editorMutationError.ts`
- Create: `src/features/application/editorMutation/editorMutationError.test.ts`
- Modify: `src/features/core/history/commandExecutor.ts:16-29`
- Modify: `src/app/i18n/locales/en-US.ts`
- Modify: `src/app/i18n/locales/zh-CN.ts`
- Test: `src/features/core/canvas/canvasPointerLoop.test.ts`

**Interfaces:**
- Consumes: commands from Task 6, capabilities from Task 7, interactions from Task 8, targeting from Task 9, and stable backend codes from Task 1.
- Produces: exact pointer routing, ordered Escape cancellation, and `presentEditorMutationError(error)` that never returns raw backend text.

- [ ] **Step 1: Add failing gesture and Escape tests**

Add pointer tests asserting:

```ts
Alt + left pin -> one DisconnectPort command
Ctrl + left connected movable pin + valid release -> one MoveConnections command
Ctrl + drag invalid release -> zero commands
plain drag replacement release -> one ConnectPins command
pointermove -> zero commands
```

Add keyboard tests asserting Escape order:

```text
drawing/moving connection
pending node creation
node drag/selection preview
node selection
Zen Mode
```

Every cancellation test must assert zero graph mutation calls.

- [ ] **Step 2: Run gesture and keyboard tests and verify red**

Run: `pnpm test -- src/features/core/canvas/canvasPointerLoop.test.ts src/features/application/editor/useEditorKeyboard.test.tsx`

Expected: FAIL because Ctrl-drag and ordered canvas cancellation are absent.

- [ ] **Step 3: Implement exact pointer-down and release routing**

In `onPinPointerDown`, handle `Alt+left` before `Ctrl+left`, then plain left:

- Alt submits one `DisconnectPort` only when `canMove` is true.
- Ctrl starts `movingConnections` only when `canMove` is true.
- Plain left starts `drawingConnection` only when `canAppend || canReplace`.

On release, submit one command only when `snappedTarget` or valid hovered target exists. Invalid move release and Escape clear the interaction without mutation. Blank ordinary connection release enters `pendingNodeCreation`; blank move release cancels.

- [ ] **Step 4: Implement Escape precedence through the shared global listener**

Before Zen Mode handling, inspect the active interaction. Cancel connection sessions first, then pending creation, then drag/selection preview, then clear selection. Call `exitZenMode()` only when all canvas levels are idle. Keep listener registration through `addGlobalEventListener()` and preserve the application modal gate.

- [ ] **Step 5: Add safe error presentation tests and implementation**

Test `presentEditorMutationError()` with every Task 1 code plus an unknown code. Assert returned/toasted text uses i18n and never contains supplied raw text such as `port UUID 123`.

Implement a code-to-i18n-key map in `editorMutationError.ts`. In `commandExecutor.ts`, log the raw structured error for diagnostics, call the presenter, and return `false`. Revision conflict recovery remains owned by `editorMutationCoordinator.ts`; do not replay destructive operations.

- [ ] **Step 6: Run focused gesture, keyboard, and error tests**

Run: `pnpm test -- src/features/core/canvas/canvasPointerLoop.test.ts src/features/application/editor/useEditorKeyboard.test.tsx src/features/application/editorMutation/editorMutationError.test.ts`

Expected: PASS; one intent is emitted only on valid release, Escape emits none, and user text contains no raw backend detail.

Run: `pnpm typecheck`

Expected: PASS.

- [ ] **Step 7: Review checkpoint**

Inspect all window/document listeners and confirm they route through `src/shared/utils/globalEvent.ts`. Confirm ordinary UI maps stable codes, while detailed addresses and internal errors appear only in logs. Do not commit.

---

### Task 11: Cross-Layer Atomicity, History, Concurrency, and Phase 1 Acceptance

**Files:**
- Modify: `src-tauri/src/project/production_tests.rs:2005-2135`
- Modify: `src/features/core/history/editorCommands.test.ts`
- Modify: `src/features/core/canvas/canvasPointerLoop.test.ts`
- Test: `src-tauri/src/project/production_tests.rs`
- Test: all focused Phase 1 frontend tests listed below

**Interfaces:**
- Consumes: all Phase 1 mutation, projection, command, interaction, targeting, and error interfaces from Tasks 1–10.
- Produces: executable acceptance evidence for one revision, one history entry, complete delta, one-step undo/redo, exact identity restoration, stale zero-effects, one-winner concurrency, and projection consistency.

- [ ] **Step 1: Add failing project-state acceptance tests**

Add tests named:

```rust
atomic_delete_nodes_commits_one_revision_and_history_entry
atomic_disconnect_variants_undo_in_one_step
connect_replacement_restores_old_connection_identity_on_undo
move_connections_undo_restores_ids_and_order_keys
failed_complex_mutation_has_zero_publication_effects
competing_same_revision_complex_mutations_commit_exactly_once
```

For each successful complex mutation, assert:

```rust
assert_eq!(result.delta.to_revision, result.delta.from_revision.next());
assert!(result.history.can_undo);
assert!(!result.history.can_redo);
assert_eq!(result.projection_replacement.projection.source_revision,
           result.delta.to_revision.get());
```

Capture document connections and order keys before mutation, undo once, and compare exact IDs/order keys. For failure and stale cases, compare document, history status, and projection snapshots before and after. For concurrency, start two same-base requests and assert exactly one `Ok` and one `StaleRevision`.

- [ ] **Step 2: Run project-state acceptance tests and verify red**

Run: `pnpm rust:test -- atomic_delete_nodes_`

Expected: FAIL until complete atomic mutation publication is wired.

Run: `pnpm rust:test -- connect_replacement_restores_`

Expected: FAIL until undo restores incumbent identity exactly.

Run: `pnpm rust:test -- competing_same_revision_complex_mutations_`

Expected: FAIL until the test fixture and complete mutation path satisfy one-winner semantics.

- [ ] **Step 3: Correct only cross-layer defects exposed by the tests**

If planner operations are correct, keep production changes in existing publication/history owners rather than adding frontend compensation. Ensure `apply_editor_graph_mutation()` records the complete patch as one transaction and returns the projection generated from committed authority. Preserve current stale-revision rejection without automatic destructive replay.

- [ ] **Step 4: Run all focused Rust Phase 1 tests**

Run:

```sh
pnpm rust:test -- delete_nodes_
pnpm rust:test -- disconnect_
pnpm rust:test -- connect_
pnpm rust:test -- move_connections_
pnpm rust:test -- projects_connection_capabilities_
pnpm rust:test -- atomic_delete_nodes_
pnpm rust:test -- atomic_disconnect_
pnpm rust:test -- connect_replacement_restores_
pnpm rust:test -- move_connections_undo_
pnpm rust:test -- failed_complex_mutation_
pnpm rust:test -- competing_same_revision_complex_mutations_
```

Expected: every command exits 0; complex actions produce one revision/history transaction and failures preserve all authority state.

- [ ] **Step 5: Run all focused frontend Phase 1 tests**

Run:

```sh
pnpm test -- src/features/core/history/editorCommands.test.ts
pnpm test -- src/shared/types/dto/editorMutationWireParser.test.ts
pnpm test -- src/features/domain/editorProjection/editorProjection.test.ts
pnpm test -- src/features/core/dataStore/graphProjectionStore.test.ts
pnpm test -- src/shared/utils/pinCompatibility.test.ts
pnpm test -- src/features/core/canvas/canvasInteractionState.test.ts
pnpm test -- src/features/core/canvas/connectionTargeting.test.ts
pnpm test -- src/features/core/canvas/canvasPointerLoop.test.ts
pnpm test -- src/features/application/editor/useEditorKeyboard.test.tsx
pnpm test -- src/features/application/editorMutation/editorMutationError.test.ts
pnpm test -- src/views/EditorView/Canvas/core/ConnectionLine.test.tsx
pnpm test -- src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx
```

Expected: every command exits 0; command tests observe one mutation, pointermove observes zero IPC, invalid/cancelled gestures observe zero mutation, and capability/feedback tests distinguish append, replace, and invalid.

- [ ] **Step 6: Run repository-wide required verification**

Run: `pnpm verify`

Expected: PASS for frontend typecheck/tests, Rust formatting/check/tests, scientific crate tests, and final diff whitespace validation.

Run: `git --no-pager diff --check`

Expected: no output and exit code 0.

- [ ] **Step 7: Final review checkpoint**

Review `git --no-pager diff --stat` and `git --no-optional-locks status --short`. Confirm only Phase 1 implementation/test files were added to the implementation diff, every pre-existing unrelated change remains untouched, old mutation wire variants and `canConnect` are absent, no frontend authoritative mutation loop remains, and no commit was created.

## Phase 1 Acceptance Matrix

- `DeleteNodes`: Tasks 2, 6, and 11.
- `DisconnectConnections`, `DisconnectPort`, `DisconnectNode`: Tasks 2, 6, and 11.
- Occupied `Single` Connect replacement and bounded `Multiple` rejection: Tasks 3 and 11.
- `MoveConnections` all-or-nothing authority resolution: Tasks 4, 6, 10, and 11.
- Projection `canAppend`, `canReplace`, `canMove`: Tasks 5 and 7.
- Contextual create-and-connect accepts appendable or replaceable sources: Tasks 3 and 5.
- Mutually exclusive connection interaction states: Task 8.
- Snapping and valid/replace/invalid feedback: Task 9.
- Ctrl-drag, Alt-disconnect, Escape precedence: Task 10.
- Stable user-facing connection errors without raw internal text: Tasks 1 and 10.
- One revision, one history entry, one-step undo/redo, stale/concurrency zero-effects, committed projection consistency: Task 11.
