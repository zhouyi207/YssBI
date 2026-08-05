# Node Architecture Final Closure Design

## Goal

Close the remaining Phase 2 and Phase 5 requirements in `docs/plan/node-architecture.md`, then publish every row in `TODO.md` under `## node_architecture 进度` at 100% using fresh verification evidence.

This work preserves the completed Phase 1, 3, 4, 6, 7, 8, and 9 contracts. It introduces no compatibility path, second Registry, second runtime, or frontend authority.

## Constraints

- Work directly on `shadcn`.
- Do not create a worktree, branch, commit, amend, tag, stage, push, reset, revert, restore, or clean.
- Preserve unrelated dirty work.
- Follow RED-GREEN for every behavior change.
- Run focused Rust tests serially with `CARGO_BUILD_JOBS=1` and `--test-threads=1`.
- Require an independent Critical/Important review before publishing each Phase at 100%.
- Keep resource paths opaque.
- Keep semantic and document authority in Rust.
- Do not reject repairable semantic errors at the persistence boundary.

## Phase 2: GraphDocument Authority Closure

### Structural validation boundary

`GraphDocument::validate()` remains the structural invariant checker. It validates document topology and persisted address integrity, including dangling nodes, missing dynamic-port bindings, invalid input-state ownership, connection cardinality, and related normalized-document invariants.

It must not require a current Registry, localization bundle, resource snapshot, or type analysis. Unknown node types and other repairable semantic problems remain loadable and appear later as structured compiler diagnostics.

Every graph installation path converges on `ProjectState::insert_graph` or its single private implementation. That boundary validates the incoming `GraphResourceDocument` before changing authoritative state. Loading, creation, duplication, import, restore, and test helpers cannot bypass this validation.

A rejected insertion has zero authoritative effects. In particular, it must not change:

- `ProjectState.project_data.graphs`;
- graph revision bookkeeping;
- project authority or publication generation;
- compile products;
- History state;
- emitted project events.

### Mutation surface

Descriptor-driven `EditorGraphMutationDto` remains the production editor write protocol. Node creation accepts an exact Registry-produced `NodeCreationDescriptor`; Rust allocates persistent identities and validates descriptor scope and resource revisions.

Raw document mutation and arbitrary patch entry points must not remain as parallel production write APIs. Test fixture construction may use test-only helpers. Internal domain code may retain narrowly scoped private helpers where they are necessary to implement the authoritative editor protocol.

Projected-member materialization remains a dedicated transaction rather than a generic patch escape hatch. It continues to:

- accept `ProjectedMemberRef`;
- validate compilation basis and authorization;
- allocate `PortInstanceId` and `ConnectionId` in Rust;
- commit the dynamic binding and connection atomically.

### Persistence contracts

Focused round-trip coverage proves that persisted input state is independent from its effective runtime binding:

1. An active connection wins over a literal override.
2. The literal remains persisted while shadowed by the connection.
3. Disconnecting restores the literal.
4. Clearing the literal reveals the protocol default.
5. Ordered multiple connections preserve `OrderKey` ordering across serialization and loading.

A separate contract proves that GraphDocument bytes are independent of locale and catalog display metadata. Persisted graph JSON contains stable semantic IDs and document UUIDs but not localized labels, category titles, projection basis values, Registry snapshot handles, or compiler-local value references.

## Phase 5: Deterministic Structured Diagnostics Closure

### Single diagnostic definition authority

Compiler diagnostics use one typed definition inventory. Each definition owns:

- a stable diagnostic code;
- its stable message key;
- default severity where applicable;
- the allowed named semantic arguments;
- default-locale message templates;
- explicit locale fallback behavior.

Compiler passes create diagnostics from stable semantic facts. They do not place a pre-rendered English sentence into a generic `detail` argument.

The design may use a typed enum, declarative macro, or static definition table, provided emitted production codes and localization validation derive from the same authority. A second manually synchronized code list is not acceptable.

### Localization boundary

`AnalysisSnapshot` stores locale-independent diagnostic data only:

- code;
- severity;
- named arguments;
- primary location;
- related locations.

Editor projection renders the message using the selected locale. Rendering under another locale changes only the display message. It does not change diagnostic identity, ordering, semantic arguments, or locations.

Built-in assembly validates that every production compiler diagnostic definition has a default-locale template. Missing required default-locale entries fail initialization with a typed error rather than silently producing an opaque fallback.

### Determinism

Compiler diagnostic ordering is canonical and independent of document insertion history. A differential invalid-graph fixture uses fixed node, connection, dynamic-port, and resource identities and produces multiple diagnostics from more than one analysis pass.

Reversed and seeded-random insertion orders must produce:

- byte-identical serialized `AnalysisSnapshot` values;
- the same exact diagnostic sequence;
- identical codes, named arguments, primary locations, and related locations;
- no localized text in the snapshot.

The existing valid-graph differential tests remain responsible for `ValidatedSemanticGraph` and `ExecutionPlan` equivalence. Invalid graphs are not required to produce those artifacts.

## Error handling

Persistence failures map structural validation errors to the existing project-format or graph-insertion error hierarchy while preserving typed sources. The error must identify that the graph document is structurally invalid without flattening the underlying validation cause into an untyped string when a typed source can be retained.

Diagnostic-definition and localization validation failures propagate through the existing typed built-in assembly chain established by Phase 1. No fallback Registry or partially initialized project state is created.

## Test and review sequence

### Phase 2

1. Add failing load and insertion tests for structurally invalid documents.
2. Implement validation at the shared insertion boundary.
3. Add failing source/API boundary tests for parallel raw write surfaces.
4. Restrict or relocate those surfaces without changing authoritative editor behavior.
5. Add persistence precedence, ordering, and locale/display-independence tests.
6. Run focused document, project I/O, ProjectState, and source-audit tests plus `pnpm rust:check` and `git diff --check`.
7. Obtain independent review.
8. Update only the Phase 2 row in `TODO.md` to 100% after clean review.

### Phase 5

1. Add failing completeness tests for emitted diagnostic definitions and default-locale templates.
2. Introduce the single typed diagnostic definition authority.
3. Migrate compiler emission away from opaque English `detail` payloads.
4. Add locale-invariance/rendering tests.
5. Add the non-empty invalid-graph insertion-order differential test.
6. Run focused compiler, catalog, projection, Registry, and contract tests plus `pnpm rust:check`, relevant frontend tests if DTOs change, and `git diff --check`.
7. Obtain independent review.
8. Update only the Phase 5 row in `TODO.md` to 100% after clean review.

### Final acceptance

After both phases have clean reviews:

- run `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm verify`;
- confirm protected node-system fixture hashes remain unchanged unless a separately reviewed contract change explicitly requires an update;
- confirm branch, HEAD, staging, and dirty-work hygiene;
- run `git diff --check` after the final ledger and TODO updates;
- record exact fresh evidence in a new SDD ledger.

## Out of scope

- Durable History across project reloads.
- Collaboration or distributed transactions.
- Incremental regional compilation.
- Runtime cache policy, deadlines, retries, parallel scheduling, or backpressure.
- Relational optimizer expansion.
- Rejecting unknown node types or other repairable semantic failures during file loading.
- Broad GraphDocument field encapsulation when shared insertion validation already protects authority.
