# Node Architecture Phase 2 Final Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every GraphDocument installation structurally validated and atomic, expose only the descriptor-driven production mutation boundary, and complete the persistence verification matrix.

**Architecture:** `GraphDocument::validate()` remains Registry-independent structural validation. File parsing, prepared activation, public insertion, lazy load, and resource-patch installation validate before any authoritative effect. Raw graph mutation helpers remain available only to Rust tests; committed patch DTOs and projected-member atomic materialization remain intact.

**Tech Stack:** Rust, serde/serde_json, Tauri project state, existing node-system document transactions, pnpm Cargo scripts.

## Global Constraints

- Work directly on `shadcn` and preserve unrelated dirty work.
- Do not create a worktree, branch, stage, commit, amend, tag, push, reset, revert, restore, or clean.
- Use RED-GREEN for behavior changes.
- Run focused Rust tests with `CARGO_BUILD_JOBS=1` and `--test-threads=1`.
- `ProjectState.project_data` remains authoritative.
- `ProjectState::insert_graph` remains the only graph insertion boundary.
- Structural validation must not consult Registry, localization, resources, types, or compiler analysis.
- Unknown node types and repairable semantic errors remain loadable.
- Do not remove `GraphDocumentPatch`; History and delta DTOs require it.
- Do not remove projected-member basis/authorization validation or Rust-owned identity allocation.
- Update Phase 2 in `TODO.md` only after focused verification and independent review are clean.

---

## File map

- `src-tauri/src/node_system/document/transaction.rs`: existing `GraphDocument::validate()` structural authority; behavior unchanged.
- `src-tauri/src/node_system/document/error.rs`: existing typed `DocumentError` source.
- `src-tauri/src/project/project_data.rs`: add validation delegation for `GraphResourceDocument` and validate bulk project data.
- `src-tauri/src/project/project_error.rs`: preserve typed structural validation sources at project I/O and filesystem boundaries.
- `src-tauri/src/project/project_io.rs`: reject structurally invalid graph files after envelope validation.
- `src-tauri/src/project/project_state.rs`: preflight every graph installation before revisions, publication, History, or compile invalidation.
- `src-tauri/src/project/project_activation.rs`: validate all prepared graphs before activation publication.
- `src-tauri/src/node_system/document/mod.rs`: stop production re-export of raw mutation/store helpers.
- `src-tauri/src/node_system/document/mutation.rs`: remove the unused public free raw-mutation wrapper; retain internal/test transaction machinery.
- `src-tauri/src/node_system/testing/source_audit.rs`: enforce the closed production write surface.
- `src-tauri/src/node_system/document/tests.rs`: preserve projected-member atomicity coverage.
- `src-tauri/src/project/production_tests.rs`: zero-authoritative-effect insertion/patch regressions.
- `TODO.md`: publish Phase 2 only after clean review.
- `.superpowers/sdd/2026-08-05-node-architecture-phase2-final-closure/progress.md`: record RED-GREEN, review, and verification evidence.

---

### Task 1: Reject invalid graph files and installations atomically

**Files:**
- Modify: `src-tauri/src/project/project_data.rs`
- Modify: `src-tauri/src/project/project_error.rs`
- Modify: `src-tauri/src/project/project_io.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/project/project_activation.rs`
- Test: `src-tauri/src/project/project_io.rs`
- Test: `src-tauri/src/project/production_tests.rs`
- Test: `src-tauri/src/project/project_activation.rs`

**Interfaces:**
- Consumes: `GraphDocument::validate(&self) -> Result<(), DocumentError>`.
- Produces: `GraphResourceDocument::validate(&self) -> Result<(), DocumentError>` and typed project-boundary errors preserving `DocumentError`.
- Preserves: `ProjectState::insert_graph(...) -> Result<GraphResourceDocument, ProjectFilesystemError>`.

- [ ] **Step 1: Add the invalid-file RED test**

Add `production_graph_io_rejects_structurally_invalid_document` beside existing graph I/O tests. Serialize a valid event envelope, then inject a fixed `DocumentConnection` whose output references a missing fixed `NodeId`. Keep schema version, graph kind, JSON syntax, and function shape valid. Assert `load_project_graph_from_file` returns the project error variant for an invalid graph document and exposes `DocumentError::EndpointNodeNotFound(missing_node_id)` as its typed source.

- [ ] **Step 2: Verify the invalid-file test fails for the expected reason**

Run:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- project::project_io::tests::production_graph_io_rejects_structurally_invalid_document --exact --test-threads=1
```

Expected: FAIL because the current parser accepts structurally invalid normalized document content.

- [ ] **Step 3: Add the public insertion zero-effect RED test**

Add `structurally_invalid_insert_graph_has_zero_authoritative_effects`. Start with one valid loaded graph and materialize a compile slot. Snapshot project JSON, graph revision state, publication/authority state, History status/head/lengths, and compile-slot presence. Attempt to replace it with a graph containing a dangling endpoint. Assert a typed structural error and exact equality of every snapshot.

- [ ] **Step 4: Verify the insertion test fails**

Run:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- project::production_tests::structurally_invalid_insert_graph_has_zero_authoritative_effects --exact --test-threads=1
```

Expected: FAIL because insertion currently changes authoritative state and compile publication bookkeeping.

- [ ] **Step 5: Add the resource-patch zero-effect RED test**

Add `structurally_invalid_resource_patch_insert_has_zero_authoritative_effects` using `ResourceDocumentPatch::InsertGraph`. Assert no graph map entry, graph revision, authority/publication generation, History change, event, or compile invalidation is produced.

- [ ] **Step 6: Verify the resource-patch test fails**

Run:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- project::production_tests::structurally_invalid_resource_patch_insert_has_zero_authoritative_effects --exact --test-threads=1
```

Expected: FAIL because the patch insertion is not structurally preflighted.

- [ ] **Step 7: Add the prepared-activation RED test**

Add `prepared_activation_rejects_structurally_invalid_graph_data`. Build `ProjectData` containing a graph with a dangling connection and call `PreparedProjectActivation::from_data`. Assert a typed invalid-graph source is returned before revision ledgers or a prepared publication are created.

- [ ] **Step 8: Verify the activation test fails**

Run:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- project::project_activation::tests::prepared_activation_rejects_structurally_invalid_graph_data --exact --test-threads=1
```

Expected: FAIL because bulk activation currently accepts the graph.

- [ ] **Step 9: Implement structural validation delegation and typed errors**

Add:

```rust
impl GraphResourceDocument {
    pub fn validate(&self) -> Result<(), DocumentError> {
        self.document.validate()
    }
}
```

Add project I/O/filesystem error variants carrying graph path and `DocumentError` as the error source. Match the project’s existing manual or derived `Display`/`Error::source` style; do not flatten with `to_string()`.

- [ ] **Step 10: Validate graph-file content after envelope checks**

In `parse_graph_resource_document`, call `document.document.validate()` only after JSON/schema/kind/function-shape validation. Map failure to the typed project error using the supplied disk path. Keep unknown `NodeTypeId` values valid.

- [ ] **Step 11: Validate every installation before effects**

Make private insertion fallible and validate before `data.graphs.insert`. In public insertion, propagate failure before graph-revision updates, authority advancement, publication changes, or compile invalidation.

Add pure preflight for every graph carried by a resource patch:

- `InsertGraph.resource`;
- `MoveGraph.moved`;
- every graph in `MoveGraph.referenced_graphs`.

Run preflight before source removal, revision mutation, History mutation, or publication. Retain defensive validation in the private insertion helper.

- [ ] **Step 12: Validate bulk activation before preparing state**

In `PreparedProjectActivation::from_data`, validate every `data.graphs` resource before deriving revision ledgers or constructing the prepared activation. Propagate through the existing project filesystem error hierarchy. Do not simulate activation by repeatedly calling public `insert_graph`, because that would alter authority-generation and invalidation behavior.

- [ ] **Step 13: Run Task 1 GREEN tests**

Run the four exact commands from Steps 2, 4, 6, and 8. Expected: all PASS with nonzero matched test counts.

- [ ] **Step 14: Run adjacent insertion/load tests**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- project::project_io::tests --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- project::project_activation::tests --test-threads=1
```

Expected: all PASS.

- [ ] **Step 15: Obtain independent Task 1 review**

Reviewer must verify validation occurs before every effect, `MoveGraph` preflight precedes removal, typed causes survive, and semantic/Registry checks did not leak into structural validation. Fix all Critical/Important findings and repeat focused verification before proceeding.

---

### Task 2: Close the raw production graph-write surface

**Files:**
- Modify: `src-tauri/src/node_system/document/mod.rs`
- Modify: `src-tauri/src/node_system/document/mutation.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/node_system/testing/source_audit.rs`
- Test: `src-tauri/src/node_system/document/tests.rs`

**Interfaces:**
- Consumes: existing `EditorGraphMutationDto`, `GraphDocumentPatch`, and projected-member transaction.
- Produces: production-visible descriptor-driven mutation only; raw helpers remain `#[cfg(test)] pub(crate)` where fixtures require them.

- [ ] **Step 1: Add the production write-surface RED audit**

Add `production_graph_write_surface_exposes_only_editor_mutations`. Parse `document/mod.rs`, `document/mutation.rs`, and `project/project_state.rs` with `syn`. Assert:

- `GraphMutation`, `RevisionedGraphStore`, and free `apply_mutation` are not public production re-exports;
- no public production free function named `apply_mutation` remains;
- no public production `ProjectState::apply_graph_mutation` or `ProjectState::apply_graph_patch` remains;
- `ProjectState::apply_editor_graph_mutation` remains public;
- `GraphDocumentPatch` remains publicly available as committed delta/History data.

- [ ] **Step 2: Verify the audit fails**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::testing::source_audit::production_graph_write_surface_exposes_only_editor_mutations --exact --test-threads=1
```

Expected: FAIL on the current public raw symbols.

- [ ] **Step 3: Restrict raw document symbols**

Remove `GraphMutation`, `RevisionedGraphStore`, and free `apply_mutation` from the production `pub use` list. Delete the unused free wrapper. Add only the test-gated crate-visible exports needed by existing Rust fixtures:

```rust
#[cfg(test)]
pub(crate) use mutation::{GraphMutation, RevisionedGraphStore};
```

Keep `GraphDocumentPatch` and `GraphDocumentOperation` production-visible.

- [ ] **Step 4: Restrict project-level raw accepting APIs**

Gate `ProjectState::apply_graph_mutation` and `ProjectState::apply_graph_patch` with `#[cfg(test)] pub(crate)`. Gate their raw-type imports similarly. Do not change editor mutation commit paths or History’s committed patch representation.

- [ ] **Step 5: Run the write-surface audit GREEN**

Run the Step 2 command. Expected: PASS.

- [ ] **Step 6: Prove projected-member behavior remains intact**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::document::tests::projected_member_rejects_a_stale_compilation_basis --exact --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::document::tests::projected_member_rejects_authorization_for_another_member --exact --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::document::tests::projected_member_materialization_and_connection_commit_atomically --exact --test-threads=1
```

Expected: 3 exact tests PASS. Confirm Rust still allocates `PortInstanceId` and `ConnectionId`, validates basis/authorization, and commits binding plus connection atomically.

- [ ] **Step 7: Run compile checking for non-test production visibility**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
```

Expected: PASS, proving production code does not depend on the test-only raw exports.

- [ ] **Step 8: Obtain independent Task 2 review**

Reviewer must distinguish accepting APIs from committed patch DTOs and confirm invalid-graph/compiler/History fixtures still have test-only construction support. Resolve all Critical/Important findings.

---

### Task 3: Complete persistence precedence and metadata contracts

**Files:**
- Test: `src-tauri/src/project/project_io.rs`
- Production changes: none unless a test reveals a genuine persistence defect.

**Interfaces:**
- Consumes: normalized `InputState`, `EffectiveInputBinding`, `OrderKey`, graph serializer/loader.
- Produces: direct verification of Phase 2 persistence-matrix claims.

- [ ] **Step 1: Add input precedence and order round-trip coverage**

Add `production_graph_io_preserves_input_precedence_and_connection_order`. Use fixed node and connection IDs. Persist one literal override and two ordered connections on one input. Assert before and after round-trip that effective connections are sorted by `OrderKey`, while the literal remains in `input_states`. Disconnect both connections and assert the literal becomes effective. Clear the literal and pass a protocol default to effective-binding resolution; assert the default becomes effective.

Do not invent a failure. This is a characterization test unless it exposes a real serialization defect.

- [ ] **Step 2: Run the precedence test**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- project::project_io::tests::production_graph_io_preserves_input_precedence_and_connection_order --exact --test-threads=1
```

Expected: PASS. If it fails, stop and root-cause the specific persistence defect before production changes.

- [ ] **Step 3: Add stable-document metadata coverage**

Add `persisted_graph_json_contains_only_stable_document_metadata`. Serialize a graph with fixed stable node type, node UUID, dynamic port instance UUID, connection UUID, and dynamic-member locator. Assert those identities are present. Recursively reject these exact keys:

```text
displayName
displayLabel
categoryTitle
localizedLabel
projection
projectionBasis
sourceRevision
registryFingerprint
registrySnapshot
snapshotHandle
resourceVersions
compilerValueRef
planValueSource
```

Permit `userLabel`, because it is user-authored document state. Assert two serializations of the same resource and local-variable map are byte-identical.

- [ ] **Step 4: Run the metadata test**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- project::project_io::tests::persisted_graph_json_contains_only_stable_document_metadata --exact --test-threads=1
```

Expected: PASS.

- [ ] **Step 5: Add unknown-node loadability coverage**

Add `production_graph_io_loads_unknown_node_types_for_compiler_diagnostics`. Persist a structurally valid node with `NodeTypeId("yssbi.test.missing")`, load it, and assert exact preservation without Registry lookup.

- [ ] **Step 6: Run the unknown-node test**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- project::project_io::tests::production_graph_io_loads_unknown_node_types_for_compiler_diagnostics --exact --test-threads=1
```

Expected: PASS.

- [ ] **Step 7: Run the broader persistence matrix**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- project::project_io::tests --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::document::tests --test-threads=1
```

Expected: all PASS.

- [ ] **Step 8: Obtain independent Task 3 review**

Reviewer must confirm the tests verify persisted authority rather than projection behavior, avoid locale/Registry coupling, and do not weaken user labels or opaque resource paths.

---

### Task 4: Verify and publish Phase 2

**Files:**
- Create: `.superpowers/sdd/2026-08-05-node-architecture-phase2-final-closure/progress.md`
- Modify after clean review only: `TODO.md`

- [ ] **Step 1: Run the focused Phase 2 matrix**

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- node_system::document::tests --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- project::project_io::tests --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm rust:test -- project::project_activation::tests --test-threads=1
```

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
```

```sh
pnpm rust:fmt:check
```

```sh
git diff --check
```

Record exact matched/passed counts and warnings.

- [ ] **Step 2: Obtain whole-slice Phase 2 review**

Request an independent review against the design and this plan. Fix every Critical/Important finding with focused RED-GREEN evidence, then repeat the relevant matrix.

- [ ] **Step 3: Create the Phase 2 ledger**

Record constraints, each task/fix round, exact commands and counts, review verdict, HEAD/branch/staging state, and preservation of unrelated dirty work.

- [ ] **Step 4: Publish Phase 2 at 100%**

Only after clean review, change only the Phase 2 row in `TODO.md` to 100%. The status must state that structural load/insertion validation, zero-effect rejection, descriptor-only production mutation, projected-member atomicity, persistence precedence/order, and localization-independent bytes are complete.

- [ ] **Step 5: Run publication hygiene**

```sh
git --no-pager diff --check
```

```sh
git --no-optional-locks status --short --branch
```

Confirm no staged changes or forbidden Git operations.
