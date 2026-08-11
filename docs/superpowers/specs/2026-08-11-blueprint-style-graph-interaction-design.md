# Blueprint-Style Graph Interaction Design

**Date:** 2026-08-11  
**Status:** Approved design  
**Scope:** Graph editor connection semantics, edge editing, reroute nodes, subgraph clipboard operations, selection, and viewport shortcuts

## 1. Goals

Bring the YssBI graph editor's high-frequency interactions closer to Unreal Engine Blueprint while preserving the project's Rust-authoritative graph architecture.

The design must provide:

- atomic replacement when connecting occupied single-capacity ports;
- atomic movement of all connections from a port;
- atomic node-group deletion and break-links operations;
- explicit connection selection and editing;
- persistent, compiler-transparent reroute nodes;
- authoritative subgraph duplicate, copy, paste, and cut behavior;
- Blueprint-style selection, focus, fit, cancellation, and keyboard shortcuts;
- one graph revision and one history entry for each user-visible graph operation;
- no frontend-authored graph patches or optimistic topology changes.

## 2. Non-goals

This work does not include:

- changing the `SetParameters` full-replacement versus partial-merge contract;
- project-wide atomic Save All behavior across resources;
- a generic composite mutation protocol;
- collaborative multi-user editing;
- automatic node layout;
- comment boxes, macros, function collapsing, or Blueprint debugging animation;
- mixed node-and-connection selection followed by one delete operation;
- automatic migration of missing resources across projects;
- replacement of the existing canvas with a second graph UI library.

## 3. Design principles

### 3.1 One user action, one authoritative transaction

A user-visible operation maps to one high-level mutation, one `GraphDocumentPatch`, one graph revision, and one Rust history entry.

Frontend loops must not combine multiple authoritative mutations into an operation that the user perceives as atomic. In particular, node-group deletion and break-all-links must not submit one request per entity.

### 3.2 The frontend sends intent, not graph operations

The frontend sends only information that Rust cannot infer from the current authoritative graph, such as selected identities, pointer placement, or clipboard content.

Rust owns:

- connection replacement decisions;
- affected connection discovery;
- node, port, and connection ID allocation;
- dynamic port bindings;
- patch operation ordering;
- validation;
- history and inverse patches;
- final graph projection.

### 3.3 Domain-specific mutations instead of generic composition

Do not expose a generic `Composite(Vec<EditorGraphMutationDto>)`. It would move ordering, shared-resource deduplication, staged validation, and temporary identity dependencies to callers.

Add high-level mutations for real user actions and let Rust expand them into ordinary graph operations.

### 3.4 Preview locally, commit once

Pointer movement, snapping, hover feedback, and connection previews use the installed frontend projection and animation frames. They must not invoke Tauri.

Only pointer release or a confirmed menu action sends one mutation. The frontend does not optimistically alter committed graph topology.

## 4. Connection replacement semantics

### 4.1 Single-capacity ports

`ConnectionsPerPort::Single` remains a strict document invariant: a materialized port may have at most one connection.

A new `Connect` intent targeting occupied single-capacity endpoints means replacement, not addition. Rust removes all incumbent connections on both occupied `Single` endpoints and inserts the requested connection in one patch.

If the output and input endpoints each have a different incumbent connection, both old connections are removed. If one old connection is incident to both endpoints, it is removed once.

This behavior is independent of drag direction.

### 4.2 Bounded multiple ports

A full `ConnectionsPerPort::Multiple { max: Some(n), .. }` port remains non-replaceable. The mutation fails when another connection would exceed its maximum.

The editor must not silently choose an existing connection to evict from a multiple-capacity port.

### 4.3 Validation and failure

Rust completes endpoint, orphan, direction, kind, type, ordering, and capacity validation before committing authority state.

Planning may use a staged `GraphDocument`, but no old connection is removed from the authoritative document unless the complete replacement patch succeeds.

A validation failure preserves:

- the original connections;
- graph revision;
- history stacks;
- installed projection.

### 4.4 Duplicate endpoint requests

Connecting an already-connected output/input pair must have an explicit non-empty behavior. The implementation should reject it as an existing connection rather than creating a replacement with a new connection ID or an empty history transaction.

## 5. Connection movement semantics

`Ctrl + drag` on a connected port moves all current connections on that port.

The request is intent-oriented:

```ts
{
  type: 'moveConnections',
  payload: {
    source: PortAddressDto,
    target: PortAddressDto,
  },
}
```

The frontend does not send the source connection IDs. Rust resolves them from the current authoritative document.

Rust determines whether each moved connection replaces its input or output endpoint, validates every proposed connection, handles replaceable occupied `Single` targets, and commits only if all connections can move.

The operation is all-or-nothing. Releasing on an invalid target or pressing `Escape` leaves all original connections unchanged.

## 6. Mutation protocol consolidation

### 6.1 Replace singular pseudo-batches

Replace:

```rust
DeleteNode { node_id }
Disconnect { connection_id }
```

with high-level mutations:

```rust
DeleteNodes {
    node_ids: Vec<NodeId>,
}

DisconnectConnections {
    connection_ids: Vec<ConnectionId>,
}

DisconnectPort {
    address: PortAddressDto,
}

DisconnectNode {
    node_id: NodeId,
}
```

Single-node deletion and single-edge deletion use arrays of length one. This 0.x project does not retain compatibility shims for the old variants.

### 6.2 Keep correctly scoped existing mutations

Retain:

- `MoveNodes`, which is already a correct atomic batch;
- `Connect`, which represents one connection gesture even when its patch replaces old edges;
- `CreateNode`, including its existing atomic `connectFrom` behavior;
- `SetParameters`, scoped to one node's parameter map;
- `SetLiteral`, scoped to one input edit;
- `AddPortInstance`, scoped to one logical member creation;
- `RemovePortInstance`, scoped to one logical member removal.

Do not create `CreateNodes` for clipboard operations. Node-group creation needs request-local references, internal edges, dynamic bindings, and newly allocated identities, so it receives dedicated subgraph mutations.

### 6.3 Collection mutation rules

All collection mutations must:

- reject empty collections;
- reject duplicate direct targets rather than hiding frontend errors;
- validate every target before committing;
- deduplicate derived shared resources such as incident connections;
- generate operations in deterministic identity order;
- produce one patch and one history transaction.

## 7. Atomic node-group deletion

`DeleteNodes` validates all requested nodes before generating removals. If any node is missing, Rust-managed, or otherwise non-deletable, the entire mutation fails.

The planner builds a node ID set, then scans the document once to collect:

- every connection incident to a selected node;
- every input state owned by a selected node;
- every dynamic port binding owned by a selected node;
- every selected node.

Operations use deterministic order:

1. remove connections by `ConnectionId`;
2. clear input states by `PortAddress`;
3. remove port bindings by `PortAddress`;
4. remove nodes by `NodeId`.

`GraphDocumentPatch::inverse()` reverses this into the valid restore order: nodes, bindings, input states, then connections.

The frontend `DeleteNodes` command sends one `deleteNodes` mutation instead of looping over node IDs.

## 8. Atomic break-links operations

- `DisconnectConnections` removes an explicitly selected edge set.
- `DisconnectPort` lets Rust derive all current connections for one port.
- `DisconnectNode` lets Rust derive all incident connections for one node.

These variants share one Rust helper that validates, sorts, deduplicates, and generates `RemoveConnection` operations.

`Alt + left click` on a pin uses `DisconnectPort`. The node menu's Break All Links action uses `DisconnectNode`. Edge deletion uses `DisconnectConnections`.

## 9. Frontend interaction state

Canvas interaction becomes an explicit mutually exclusive state instead of accumulating optional booleans:

```ts
type CanvasInteraction =
  | { type: 'idle' }
  | { type: 'selecting'; session: SelectionSession }
  | { type: 'draggingNodes'; session: NodeDragSession }
  | { type: 'drawingConnection'; session: ConnectionDrawSession }
  | { type: 'movingConnections'; session: ConnectionMoveSession }
  | { type: 'pendingNodeCreation'; session: PendingNodeCreationSession };
```

A connection draw session tracks the start port, pointer, hovered target, snapping target, compatibility result, and whether a successful drop will append or replace.

A connection move session tracks the source port and local preview information. The backend remains responsible for resolving the authoritative moved connection set.

### 9.1 Connection feedback

The editor distinguishes:

- appendable targets;
- replaceable occupied `Single` targets;
- invalid targets with a structured reason.

Valid nearby pins snap the preview endpoint. Replacement previews highlight the target and the old edges that the installed projection indicates will be displaced. This highlight is advisory; Rust recomputes the actual replacement set.

An invalid drop sends no mutation.

### 9.2 Escape precedence

`Escape` cancels in this order:

1. active connection drawing or movement;
2. pending node creation and its palette;
3. drag or selection previews;
4. current node or edge selection;
5. page-level behavior such as leaving Zen Mode.

Global keyboard listeners continue to use `src/shared/utils/globalEvent.ts`.

## 10. Projection connection capabilities

Replace the ambiguous projection field:

```ts
canConnect: boolean
```

with Rust-authored capabilities:

```ts
connections: {
  current: number;
  maximum: number | null;
  ordered: boolean;
  canAppend: boolean;
  canReplace: boolean;
  canMove: boolean;
}
```

- `canAppend` means another edge can be added without deleting an old edge.
- `canReplace` means an occupied, non-orphan `Single` port can participate in replacement.
- `canMove` means the non-orphan port currently has movable connections.

A full bounded `Multiple` port is not replaceable. The frontend uses these values for interaction and visuals only. Every mutation is revalidated by Rust.

Contextual node catalog and atomic create-and-connect must accept a source that can append or replace. They must not reject an occupied replaceable `Single` source before the replacement planner runs.

## 11. Edge interaction and selection

Connections receive transparent wide SVG hit paths while visible paths remain thin. Hit paths support hover, click, context menu, and double click without replacing the current canvas renderer.

Selection state distinguishes nodes and connections:

```ts
interface GraphSelection {
  nodeIds: Set<string>;
  connectionIds: Set<string>;
}
```

For the first version, node and connection selections are mutually exclusive:

- selecting a node clears edge selection;
- selecting an edge clears node selection;
- `Ctrl` or `Shift` toggles items within the active selection kind;
- box selection selects nodes only;
- `Delete` sends either one `DeleteNodes` or one `DisconnectConnections` mutation.

Mixed selection deletion is deferred until it has a dedicated atomic mutation.

## 12. Reroute nodes

Reroute is a persisted `GraphDocument` node, not a frontend-only bend point.

Register separate built-in protocols for data and effect/control kinds. A data reroute uses a shared generic type parameter between one single-capacity input and one multiple-capacity output. Effect/control reroutes use matching protocol kinds with one input and one output.

Reroute nodes have stable IDs, positions, compact editor rendering, no parameter editor, and normal select, move, duplicate, delete, save, and history behavior.

### 12.1 Insert reroute mutation

```rust
InsertReroute {
    connection_id: ConnectionId,
    position: NodePosition,
}
```

Rust resolves the original edge kind and generates:

1. remove the original connection;
2. insert the appropriate reroute node;
3. connect the original output to reroute input;
4. connect reroute output to the original input.

If the original edge targets an ordered input, its `OrderKey` stays on the reroute-output-to-original-input edge. The source-to-reroute edge does not inherit target ordering.

Undo restores the original connection and original `ConnectionId`.

### 12.2 Compiler transparency

Reroute nodes remain in document and editor projections but are transparent to type and execution planning. Compiler normalization or equivalent analysis collapses reroute chains before runtime planning while preserving type constraints, dependency direction, and cycle detection.

Reroute nodes must not create runtime identity operations.

## 13. Subgraph duplicate, copy, paste, and cut

### 13.1 Export

A backend query exports selected nodes into a portable `ClipboardSubgraphDto`.

It contains:

- portable node creation identity;
- parameters and user labels;
- positions relative to subgraph bounds;
- portable dynamic binding identities;
- input literal states;
- internal connections among selected nodes;
- reroute nodes.

It excludes:

- authoritative node, port instance, and connection IDs;
- graph revision;
- external connections to unselected nodes;
- projection-only display data;
- compiler-local or runtime state.

Clipboard-local IDs express references within the snapshot.

### 13.2 Duplicate

```rust
DuplicateSubgraph {
    node_ids: Vec<NodeId>,
    offset: NodePosition,
}
```

Rust reuses its export and instantiation logic internally, generates fresh identities, preserves internal connections, excludes external connections, and commits one patch.

### 13.3 Paste

```rust
InsertSubgraph {
    snapshot: ClipboardSubgraphDto,
    anchor: NodePosition,
}
```

Clipboard content is the necessary exception to minimal frontend payloads because it may originate outside the current graph authority. Rust strictly validates the snapshot, resource references, protocol availability, size limits, local identities, dynamic bindings, literals, and internal edges before committing.

All inserted document identities are newly allocated by Rust. Partial paste is forbidden.

### 13.4 Cut

Cut uses the safe sequence:

1. export subgraph;
2. write clipboard successfully;
3. submit one `DeleteNodes` mutation.

Clipboard failure leaves the graph unchanged. Delete failure leaves the graph unchanged while retaining a usable clipboard copy, and the UI reports that copying succeeded but deletion failed.

### 13.5 Post-operation selection

After duplicate or paste, the frontend extracts inserted node IDs from the committed mutation delta after projection installation and selects those nodes. It does not predict IDs.

## 14. Keyboard and viewport behavior

- `Ctrl+A`: select all selectable nodes in the active graph.
- `F`: focus the current selection.
- `Home`: fit the complete graph.
- `Ctrl+C`: export selected subgraph to clipboard.
- `Ctrl+V`: insert clipboard subgraph near the pointer or canvas center.
- `Ctrl+D`: duplicate selected subgraph with a fixed offset.
- `Delete` / `Backspace`: delete the active node or edge selection.
- `Alt + left click pin`: atomically disconnect the port.
- `Ctrl + drag pin`: atomically move all port connections.
- `Escape`: follow the cancellation precedence in section 9.2.

`F` and `Home` change viewport state only and do not create graph history entries.

Shift box selection must union its hit nodes with the selection captured at session start, matching Shift-click semantics.

## 15. Error handling

Backend conflicts use stable codes rather than exposing raw internal text and UUIDs to users. Relevant categories include:

- port not found;
- orphan port;
- endpoint direction mismatch;
- endpoint kind or type mismatch;
- connection limit reached;
- ordered connection requirement;
- managed node deletion forbidden;
- clipboard or subgraph invalid;
- referenced resource unavailable;
- stale revision.

Frontend i18n maps these codes to actionable messages. Detailed addresses and underlying errors remain available in logs.

Mutation failure preserves the original graph and selection. Stale revision recovery refreshes authority state but does not automatically replay a destructive mutation.

## 16. Delivery phases

### Phase 1: Atomic graph operations and core wiring

- consolidate delete and disconnect mutations;
- implement `Single` replacement in `Connect`;
- add `MoveConnections`;
- project append, replace, and move capabilities;
- unify connection interaction states;
- implement Ctrl-drag, Alt-disconnect, Escape, snapping, and valid/replace/invalid feedback;
- remove raw connection errors from ordinary UI.

Phase 1 is accepted when all complex actions are atomic, undo once, preserve old topology on failure, and no frontend command loops over authoritative graph mutations.

### Phase 2: Edge editing and reroute

- add edge hit paths and edge selection;
- add edge delete and context menu;
- add `InsertReroute` and compact reroute rendering;
- implement compiler-transparent reroute analysis.

Phase 2 is accepted when data and effect/control reroutes preserve graph semantics, ordering, undo identity, and compilation behavior.

### Phase 3: Subgraph and canvas efficiency

- add export, duplicate, insert, copy, paste, and cut flows;
- add Ctrl+A, F, Home, and Ctrl+D;
- fix Shift box selection;
- hide permanently disabled menu items that remain unsupported.

Phase 3 is accepted when subgraph operations preserve internal edges, exclude external edges, allocate fresh identities, commit atomically, and integrate with authoritative projection selection.

## 17. Testing strategy

### 17.1 Rust mutation tests

Cover:

- occupied single input and output replacement;
- two independently occupied single endpoints;
- shared incumbent edge deduplication;
- bounded multiple rejection;
- ordered edge preservation;
- type, kind, direction, orphan, and stale failures with zero side effects;
- duplicate endpoint rejection;
- atomic node-group deletion including internal edges and managed nodes;
- atomic connection, port, and node disconnection;
- all-or-nothing multi-connection movement;
- data and effect/control reroute insertion and inverse;
- subgraph identity allocation, internal edges, dynamic bindings, literals, reroutes, malformed snapshots, and missing resources.

### 17.2 History and concurrency tests

For every complex mutation, verify:

- one revision increment;
- one history entry;
- complete operation delta;
- one-step undo and redo;
- exact restoration of old connection IDs and order keys;
- stale revision has no side effects;
- competing same-revision mutations allow exactly one commit;
- returned projection matches committed authority state.

### 17.3 Compiler tests

Verify reroute transparency, generic type propagation, dependency direction, chain collapse, cycle visibility, and absence of runtime reroute steps. Loading an invalid over-capacity document must continue to emit the existing connection-limit diagnostic.

### 17.4 Frontend tests

Extend pointer-loop, compatibility, command-wire, edge component, clipboard, selection, and viewport tests to cover:

- ordinary, replacement, invalid, snapped, and cancelled connection gestures;
- Ctrl-drag and Alt-disconnect;
- one mutation per delete or disconnect command;
- connection hover, select, context menu, delete, and double-click reroute;
- Escape precedence;
- copy, paste, cut, duplicate, and committed-delta selection;
- Ctrl+A, F, Home, and Shift box union behavior;
- effect/control ports obeying Rust-authored connection capabilities.

## 18. Performance and safety

Pointer movement performs no IPC and remains animation-frame throttled. Proximity snapping uses cached pin geometry or canvas-scoped DOM geometry.

Clipboard parsing is treated as untrusted input. Rust enforces limits for node count, connection count, binding count, serialized parameter size, and nested value depth. Exact limits are selected during implementation from existing IPC and graph constraints and are covered by boundary tests.

Complex mutations continue returning one authoritative projection replacement. This feature does not introduce a second incremental synchronization protocol.

## 19. Follow-up work

Separate future designs should address:

- the `SetParameters` complete-map versus partial-update contract;
- Save All partial-success versus project-level transaction semantics;
- mixed node and edge selection operations;
- advanced Blueprint constructs beyond the interaction scope defined here.
