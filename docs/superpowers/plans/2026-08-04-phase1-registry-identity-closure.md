# Phase 1 Registry and Stable Identity Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close Phase 1 by making stable Rust `NodeTypeId` values and one validated executable `NodeRegistry` the only production node identity/registration authority, with startup localization/provenance validation and Rust-generated cross-language contracts.

**Architecture:** Frontend production behavior compares only constants from one pure stable-identity module or exact Rust descriptors. Rust freezes only executable leaf or compiler-recognized structural nodes, retains provider ownership, validates the Registry and default locale as one fallible startup bundle, and removes the old label-derived Registry path. Checked-in Rust-generated JSON fixtures and strict TypeScript parsers prove each purpose-specific wire contract, while semantic fingerprints remain locale/display independent.

**Tech Stack:** Rust 2024, Serde/serde_json, Tauri 2, TypeScript 5.8, React 19, Vitest 4, pnpm 10.

## Global Constraints

- Work directly on existing branch `shadcn`; do not create a worktree, branch, commit, or tag.
- Do not commit at the end of any task. Every task ends with a clean independent review gate, not a commit step.
- Preserve all unrelated dirty work. Before each task, record `git --no-optional-locks status --short`; stage nothing and restrict edits/review to that task's file list.
- Every numbered checkbox is a 2-5 minute operator action: write one focused test, run one named command/filter, edit one named interface/file slice, or inspect one named result. A command may continue running after launch. When a checkbox names an explicit file/symbol list, process one listed file/symbol per 2-5 minute iteration and record each iteration as a child checkbox in the SDD ledger before closing the parent checkbox; do not combine unnamed follow-up work.
- Rust is the only authority for node protocol, implementation capability, provider ownership, localization inventory, descriptors, graph state, and wire DTO production.
- Frontend production code must never reconstruct node identity from title, category, localization key, resource path, or descriptor kind.
- The exact resource node IDs are `yssbi.project.function.call`, `yssbi.project.variable.get`, `yssbi.project.variable.set`, and `yssbi.dataframe.source.get`.
- The production identity strings `Functions:Call Function`, `Variables:Get Variable`, `Variables:Set Variable`, and `Data:Get DataFrame` are forbidden. No alias, migration shim, fallback resolver, or dual matching path is permitted.
- A frozen `RegisteredNode` is exactly one of: leaf with one lowerer and no structural role, or structural with one compiler-recognized role and no lowerer.
- Remove `StaticNodeProtocol`, `StaticNodeCatalogProtocol`, `StaticPortSpec`, `NodeProtocol::from_static`, `RegisteredNode::protocol_only`, and `RegisteredNode::protocol_only_static`; test providers build complete owned `NodeProtocol` values.
- Keep the compiler's defensive missing-implementation diagnostic for corrupt/manually fabricated snapshots; normal Registry construction must be unable to produce that state.
- Registry/default-locale startup failures are typed and occur before `ProjectState` exposes editable state.
- Provider registration order must not affect ownership lookup or Registry fingerprint.
- Resource paths remain opaque backend-issued strings; frontend code forwards exact `NodeCreationDescriptor` values and never synthesizes resource parameters.
- Do not add a TypeScript code-generation package or another UI library.
- Do not change scheduling, graph execution semantics, relational behavior, structured control, plugin loading, provider trust policy, or scientific algorithms except to move identity-neutral live types/functions out of the deleted old Registry catalog.
- Run all focused Rust tests serially from the repository root with `CARGO_BUILD_JOBS=1` and `--test-threads=1`; do not use ad-hoc Cargo commands from `src-tauri` and do not run the full Rust suite before Task 6.
- For frontend changes, run exact Vitest files plus `pnpm typecheck`. For Rust changes, run focused tests plus `pnpm rust:check` and `pnpm rust:fmt:check`.
- Run `git diff --check` after every GREEN task.
- Create/update `.superpowers/sdd/2026-08-04-phase1-registry-identity-closure/progress.md` and update `TODO.md` under `## node_architecture 进度` only after that task has a clean independent spec-and-quality review. A review with any Critical or Important finding is not clean.
- Ledger entries use this exact shape: task number/title, reviewed diff paths, RED command/output summary, GREEN command/output summary, reviewer result, resolved findings, and contracts handed to the next task.
- Phase 1 remains below 100% until Task 6's whole-slice reviewer reports no Critical or Important findings and fresh `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm verify` passes.

---

## File Responsibility Map

### Frontend stable identity

- Create `src/features/domain/nodeCatalog/identity.ts`: the only frontend-owned stable built-in node ID constants and pure predicates.
- Create `src/features/domain/nodeCatalog/identity.test.ts`: constant values and negative legacy-identity behavior.
- Modify `src/features/domain/nodeCatalog/index.ts`: export stable identity types/constants/predicates.
- Modify `src/features/domain/nodeCatalog/types.ts`: remove the unused label-based `RESOURCE_SPAWNED_NODE_TYPES`; keep only live catalog item types/helpers.
- Delete `src/features/domain/nodeDefinition/resolveEffectiveDefinition.ts`, `resolveEffectiveDefinition.test.ts`, and `index.ts`: remove frontend Call Function pin reconstruction and its compatibility identity.
- Modify `src/features/core/dataStore/useNodeView.ts`, `src/features/domain/graphDiagnostics/callFunctionDiagnostics.ts`, `src/features/application/editor/cascadeGraphPathReferences.ts`, and `src/features/application/editorMutation/projectPublicationMovePlan.ts`: compare stable Call Function identity.
- Modify `src/views/EditorView/Nodes/DefaultNodeLayout.tsx`: compare stable variable/database identities for resource pin presentation.
- Modify `src/features/application/editor/canvasDrop/variableDrop.ts` and `src/features/application/editor/useCanvasDrop.ts`: route variable get/set choices by stable IDs and exact descriptors.
- Modify `src/shared/utils/pinCompatibility.test.ts`: use Rust-shaped projected ports instead of `resolveEffectiveDefinition`.
- Create `src/services/nodeSystem/nodeIdentityArchitectureContract.test.ts`: scan frontend production sources for forbidden label identities, deleted compatibility resolvers, and resource identity synthesis.
- Modify focused existing tests beside the production files above to begin with stable Rust-shaped projection/descriptors and prove legacy labels no longer match.

### Executable Registry and owned protocol construction

- Modify `src-tauri/src/node_system/registry/model.rs`: make invalid capability states unconstructible through public constructors.
- Modify `src-tauri/src/node_system/registry/validation.rs`: reject both invalid capability combinations defensively.
- Modify `src-tauri/src/node_system/registry/mod.rs`: remove protocol-only/static constructors and static protocol imports.
- Modify `src-tauri/src/node_system/protocol/model.rs` and `protocol/mod.rs`: remove the lossy static compatibility model/conversion.
- Create `src-tauri/src/node_system/testing/protocol.rs`: complete owned `NodeProtocol`/provider test builders with explicit semantic fields.
- Modify `src-tauri/src/node_system/testing/mod.rs`: expose the test builders only to crate tests.
- Modify `src-tauri/src/node_system/compiler/tests.rs`, `compiler/tests_dynamic.rs`, `document/tests.rs`, `document/tests/editor_mutation_validation.rs`, `registry/tests.rs`, and any compile error identified by the static-symbol audit: use complete owned protocols.

### Startup localization and provenance

- Modify `src-tauri/src/node_system/registry/model.rs`, `validation.rs`, and `mod.rs`: freeze immutable `NodeTypeId -> ProviderId` and `TypeId -> ProviderId` indexes and expose read-only lookup methods.
- Modify `src-tauri/src/node_system/catalog/builtin.rs`: create one fallible built-in bundle factory instead of validating/freezing through separate `expect` paths.
- Modify `src-tauri/src/node_system/catalog/localization.rs`: expose default-locale/alias validation needed by the bundle boundary without exposing localized metadata to Registry provenance.
- Modify `src-tauri/src/node_system/catalog/tests.rs` and `registry/tests.rs`: startup failure, ownership completeness, duplicate behavior, and order-independence tests.
- Modify `src-tauri/src/project/project_store.rs`: construct only from a validated built-in bundle and expose `ProjectStore::try_new`.
- Modify `src-tauri/src/project/project_state.rs`: expose `ProjectState::try_new` and avoid creating editable state before bundle success.
- Modify `src-tauri/src/lib.rs`: install fallibly constructed `ProjectState` in Tauri setup before frontend project state is available.

### Old Rust Registry and label identity removal

- Create `src-tauri/src/sci/models/mod.rs`, `regression.rs`, and `panel_did.rs`: identity-neutral live statistical DTO/model ownership currently exported from the old node catalog.
- Modify `src-tauri/src/sci/mod.rs`: export the focused model modules.
- Move the live `OLSResult`, OLS/Logit/Probit/Prais model/config/VCE structs, `ComputeDidFakeGroupRequest`, `DidFakeGroupEnginePayload`, `DidPlaceboFakeGroupBlock`, and `compute_fake_group_ri` definitions/logic from `graph/register/catalog/dataframe/` into those `sci/models/` files without importing `node_system`, `graph::register`, `NodeDefinition`, or display identity.
- Modify `src-tauri/src/commands/command_panel_did.rs`, `execution/source_builder.rs`, and `execution/struct_json.rs`: import the identity-neutral `sci::models` types.
- Delete `src-tauri/src/graph/register/` after live identity-neutral consumers move.
- Delete `src-tauri/src/graph/node/node_definition.rs` and remove its exports.
- Remove old Registry-dependent graph authoring/runtime modules from production module declarations in `src-tauri/src/graph/mod.rs`, `graph/node/mod.rs`, and `src-tauri/src/execution/mod.rs`; retain only independently used value/pin/result-source modules.
- Delete or migrate old `GraphInstance`/dynamic-pin tests so no `cfg(test)` dependency keeps the old Registry alive.
- Delete display-name reconciliation code in `src-tauri/src/graph/core/graph_data_state.rs` and `graph/core/graph_instance/{dynamic_pins,lifecycle,nodes,schema}.rs` with the obsolete module path.
- Modify `src-tauri/src/schema/node.rs` and any remaining production consumer from the source audit so no `NodeDefinition`/old Registry identity survives.
- Create `src-tauri/src/node_system/testing/source_audit.rs`: production-source audit for second registries, category/name identity, old imports, placeholder definitions, and label-based dynamic pin matching.

### Golden contracts and fingerprint matrix

- Create `src/tests/fixtures/node-system-contracts/semantic-protocol.json`.
- Create `src/tests/fixtures/node-system-contracts/i18n-inventory.json`.
- Create `src/tests/fixtures/node-system-contracts/localized-catalog.json`.
- Create `src/tests/fixtures/node-system-contracts/editor-projection.json`.
- Create `src/tests/fixtures/node-system-contracts/fingerprint-wire.json`.
- Create `src-tauri/src/node_system/testing/contracts.rs`: deterministic Rust fixture generation/comparison and explicit update switch.
- Modify `src-tauri/src/node_system/testing/mod.rs` and `tests.rs`: register focused contract tests.
- Modify `src-tauri/src/node_system/registry/tests.rs`: table-driven duplicate and fingerprint sensitivity/insensitivity matrix.
- Modify `src-tauri/src/node_system/analysis/projection.rs`: serialize editor projection Registry fingerprints as canonical lowercase 64-character hex strings.
- Modify `src/shared/types/dto/editorProjection.ts`: make `registryFingerprint` a string and add strict complete DTO guards.
- Modify `src/features/domain/editorProjection/validateProjection.ts`: accept only a guard-validated wire DTO before semantic coherence checks.
- Modify `src/services/nodeSystem/graphProjectionService.ts`: parse both load/hydrate responses through the real strict parser.
- Create `src/services/nodeSystem/nodeSystemGoldenContracts.test.ts`: import every Rust-generated fixture and test real guards/services plus malformed mutations.
- Modify existing Catalog/editor projection/service tests only where the canonical fingerprint encoding changes.

### Delivery evidence

- Create `.superpowers/sdd/2026-08-04-phase1-registry-identity-closure/progress.md` after Task 1 review passes; append only after later clean reviews.
- Modify `TODO.md` only after each clean independent review; set Phase 1 to 100% only in Task 6 after whole-slice review and fresh verification.
- Modify this plan's checkboxes/ledger references only as execution evidence; do not add commits or tags.

---

### Task 1: Frontend Stable Identity Closure

**Files:**
- Create: `src/features/domain/nodeCatalog/identity.ts`
- Create: `src/features/domain/nodeCatalog/identity.test.ts`
- Create: `src/services/nodeSystem/nodeIdentityArchitectureContract.test.ts`
- Modify: `src/features/domain/nodeCatalog/index.ts`
- Modify: `src/features/domain/nodeCatalog/types.ts`
- Delete: `src/features/domain/nodeDefinition/index.ts`
- Delete: `src/features/domain/nodeDefinition/resolveEffectiveDefinition.ts`
- Delete: `src/features/domain/nodeDefinition/resolveEffectiveDefinition.test.ts`
- Modify: `src/features/core/dataStore/useNodeView.ts`
- Modify: `src/features/domain/graphDiagnostics/callFunctionDiagnostics.ts`
- Modify: `src/features/domain/graphDiagnostics/callFunctionDiagnostics.test.ts`
- Modify: `src/features/application/editor/cascadeGraphPathReferences.ts`
- Modify: `src/features/application/editor/cascadeGraphPathReferences.test.ts`
- Modify: `src/features/application/editorMutation/projectPublicationMovePlan.ts`
- Modify: `src/features/application/editorMutation/projectPublicationMovePlan.test.ts`
- Modify: `src/views/EditorView/Nodes/DefaultNodeLayout.tsx`
- Create: `src/views/EditorView/Nodes/DefaultNodeLayout.test.tsx`
- Modify: `src/features/application/editor/canvasDrop/variableDrop.ts`
- Create: `src/features/application/editor/canvasDrop/variableDrop.test.ts`
- Modify: `src/features/application/editor/useCanvasDrop.ts`
- Modify: `src/features/application/editor/canvasDrop/spawnFromTemplate.test.ts`
- Modify: `src/shared/utils/pinCompatibility.test.ts`

**Interfaces:**
- Consumes: Rust-issued `EditorNodeProjectionDto.nodeTypeId` and exact `NodeCreationDescriptorDto` values.
- Produces:

```ts
export type NodeTypeId = string;

export const BUILTIN_NODE_TYPE_IDS = {
  callFunction: 'yssbi.project.function.call',
  getVariable: 'yssbi.project.variable.get',
  setVariable: 'yssbi.project.variable.set',
  getDataframe: 'yssbi.dataframe.source.get',
} as const satisfies Record<string, NodeTypeId>;

export type VariableNodeTypeId =
  | typeof BUILTIN_NODE_TYPE_IDS.getVariable
  | typeof BUILTIN_NODE_TYPE_IDS.setVariable;

export function isCallFunctionNodeType(value: string | undefined): boolean;
export function isVariableNodeType(value: string | undefined): boolean;
export function isDatabaseResourceNodeType(value: string | undefined): boolean;
```

- The four forbidden legacy strings return `false` from every predicate.
- Variable canvas actions select an already-issued descriptor whose `creation.nodeTypeId` equals the requested stable ID; they never build `variables/{id}` or parameter maps.

- [ ] **Step 1: Record task scope before editing**

Run:

```sh
git --no-optional-locks status --short
git --no-pager diff -- src/features/domain/nodeCatalog src/features/domain/nodeDefinition src/features/core/dataStore/useNodeView.ts src/features/domain/graphDiagnostics src/features/application/editor src/features/application/editorMutation src/views/EditorView/Nodes src/services/nodeSystem
```

Save the output in the task review packet so unrelated pre-existing changes are not attributed to this task.

- [ ] **Step 2: Write the RED stable identity unit test**

Create `identity.test.ts` with exact assertions:

```ts
expect(BUILTIN_NODE_TYPE_IDS).toEqual({
  callFunction: 'yssbi.project.function.call',
  getVariable: 'yssbi.project.variable.get',
  setVariable: 'yssbi.project.variable.set',
  getDataframe: 'yssbi.dataframe.source.get',
});
expect(isCallFunctionNodeType('Functions:Call Function')).toBe(false);
expect(isVariableNodeType('Variables:Get Variable')).toBe(false);
expect(isVariableNodeType('Variables:Set Variable')).toBe(false);
expect(isDatabaseResourceNodeType('Data:Get DataFrame')).toBe(false);
```

- [ ] **Step 3: Convert behavior tests to Rust-shaped stable projections**

Update/add focused cases proving:

```ts
const call = makeEditorProjectionFixture({
  graphPath,
  nodeId: 'call-1',
  nodeTypeId: 'yssbi.project.function.call',
  title: 'Localized call title',
});
```

Use this shape in Call title projection, missing-target diagnostics, rename cascade, and move planning. Add one legacy-labeled node to each classifier test and assert it is ignored.

- [ ] **Step 4: Add RED resource layout and variable-drop tests**

In `DefaultNodeLayout.test.tsx`, render stable get/set/database nodes and assert resource pin names are projected; rerender each with its old label and assert no resource override occurs. In `variableDrop.test.ts`, assert Alt maps to `yssbi.project.variable.set`, Ctrl/pin maps to `yssbi.project.variable.get`, and no modifier maps to `menu`.

- [ ] **Step 5: Add the RED frontend production-source audit**

`nodeIdentityArchitectureContract.test.ts` recursively scans `src/` `.ts/.tsx` files, excludes `*.test.*`, `src/tests/fixtures/`, and the audit file itself, and reports `relativePath:line`. Reject these tokens assembled with `join`/concatenation so the audit source does not self-match:

```text
Functions:Call Function
Variables:Get Variable
Variables:Set Variable
Data:Get DataFrame
resolveEffectiveDefinition
signatureToPinSlots
defaultFunctionSignature
@/features/domain/nodeDefinition
```

Also reject production code that combines `'variables/' +`, `` `variables/${...}` ``, or `{ nodeTypeId, resourcePath, createArgs }` inside canvas-drop handlers instead of forwarding a descriptor.

- [ ] **Step 6: Run the RED frontend files**

Run:

```sh
pnpm test src/features/domain/nodeCatalog/identity.test.ts src/features/domain/graphDiagnostics/callFunctionDiagnostics.test.ts src/features/application/editor/cascadeGraphPathReferences.test.ts src/features/application/editorMutation/projectPublicationMovePlan.test.ts src/views/EditorView/Nodes/DefaultNodeLayout.test.tsx src/features/application/editor/canvasDrop/variableDrop.test.ts src/services/nodeSystem/nodeIdentityArchitectureContract.test.ts
```

Expected: FAIL because `identity.ts` is absent and production sources still contain forbidden label identities/resolvers.

- [ ] **Step 7: Implement the pure identity module**

Create exactly the constants/types/predicates in the Interfaces block. Export them from `nodeCatalog/index.ts`. Do not import React, Zustand, services, DTO parsers, resource selectors, or localization into `identity.ts`.

- [ ] **Step 8: Migrate Call Function classifiers**

Replace imports from `features/domain/nodeDefinition` in `useNodeView.ts`, `callFunctionDiagnostics.ts`, `cascadeGraphPathReferences.ts`, and `projectPublicationMovePlan.ts` with `isCallFunctionNodeType` or `BUILTIN_NODE_TYPE_IDS.callFunction`. Preserve opaque resource path normalization and current title/diagnostic/move behavior.

- [ ] **Step 9: Migrate layout classifiers**

Replace `isVariableNode`/`isDataframeNode` label checks in `DefaultNodeLayout.tsx` with the pure stable predicates. Keep resource names as display-only pin labels and do not use them as identities.

- [ ] **Step 10: Migrate variable canvas choices without synthesizing descriptors**

Change `VariableNodeType` to `VariableNodeTypeId`. When a menu action is chosen, find the exact current Catalog resource item by opaque resource identity already carried by the sidebar/menu state and exact stable node ID, then pass `item.creation` unchanged to `spawnNodeFromTemplate`. If no exact descriptor exists, call `refreshCatalog()` and show `RESOURCE_CATALOG_REFRESH_MESSAGE`; do not fall back to legacy node type or parameter construction.

- [ ] **Step 11: Remove frontend dynamic Call definition reconstruction**

Delete `features/domain/nodeDefinition/`. Rewrite `pinCompatibility.test.ts` to pass `pinSlots` built from a Rust-shaped editor projection fixture; the test may verify auto-connect UI behavior but must not reconstruct a function signature or dynamic ports.

- [ ] **Step 12: Remove the unused label-based Catalog set**

Delete `RESOURCE_SPAWNED_NODE_TYPES` and its export. Keep `NodeCatalogItem`/`catalogItemKey` only if `git --no-pager grep` shows a live consumer; otherwise delete `types.ts` and remove its exports in the same step.

- [ ] **Step 13: Run GREEN frontend behavior and audit tests**

Run:

```sh
pnpm test src/features/domain/nodeCatalog/identity.test.ts src/features/domain/graphDiagnostics/callFunctionDiagnostics.test.ts src/features/application/editor/cascadeGraphPathReferences.test.ts src/features/application/editorMutation/projectPublicationMovePlan.test.ts src/views/EditorView/Nodes/DefaultNodeLayout.test.tsx src/features/application/editor/canvasDrop/variableDrop.test.ts src/features/application/editor/canvasDrop/spawnFromTemplate.test.ts src/shared/utils/pinCompatibility.test.ts src/services/nodeSystem/nodeIdentityArchitectureContract.test.ts
pnpm typecheck
git diff --check
```

Expected: all listed tests PASS, typecheck PASS, and no whitespace errors.

- [ ] **Step 14: Independently review Task 1; do not commit**

Reviewer checks every production match for node identity, exact descriptor forwarding, negative legacy cases, source-audit scope, domain dependency direction, and absence of frontend dynamic Call/interface reconstruction. Resolve every Critical/Important finding and rerun Step 13.

- [ ] **Step 15: Publish Task 1 evidence only after clean review**

Create `.superpowers/sdd/2026-08-04-phase1-registry-identity-closure/progress.md` with the Global Constraints ledger shape and append Task 1 evidence. Update only the Phase 1 row in `TODO.md` to describe reviewed frontend stable identity closure; keep completion below 100%. Do not commit.

---

### Task 2: Registry Executable Invariant and Static API Removal

**Files:**
- Modify: `src-tauri/src/node_system/registry/model.rs`
- Modify: `src-tauri/src/node_system/registry/validation.rs`
- Modify: `src-tauri/src/node_system/registry/mod.rs`
- Modify: `src-tauri/src/node_system/protocol/model.rs`
- Modify: `src-tauri/src/node_system/protocol/mod.rs`
- Create: `src-tauri/src/node_system/testing/protocol.rs`
- Modify: `src-tauri/src/node_system/testing/mod.rs`
- Modify: `src-tauri/src/node_system/registry/tests.rs`
- Modify: `src-tauri/src/node_system/compiler/tests.rs`
- Modify: `src-tauri/src/node_system/compiler/tests_dynamic.rs`
- Modify: `src-tauri/src/node_system/document/tests.rs`
- Modify: `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs`
- Modify: any additional test file reported by the exact static API source audit

**Interfaces:**
- Consumes: complete owned `NodeProtocol`, `LeafImplementation`, and `StructuralNodeRole`.
- Produces only these public constructors:

```rust
impl RegisteredNode {
    pub fn leaf(
        protocol: Arc<NodeProtocol>,
        implementation: impl Into<LeafImplementation>,
    ) -> Self;

    pub fn structural(
        protocol: Arc<NodeProtocol>,
        role: StructuralNodeRole,
    ) -> Self;
}
```

- Test construction uses:

```rust
pub(crate) struct TestProtocolBuilder { /* explicit defaults owned by testing */ }

impl TestProtocolBuilder {
    pub(crate) fn new(type_id: &str, category_id: &str) -> Self;
    pub(crate) fn ports(self, ports: Vec<PortSpec>) -> Self;
    pub(crate) fn parameters(self, parameters: Vec<ParameterSpec>) -> Self;
    pub(crate) fn execution(self, execution: ExecutionSemantics) -> Self;
    pub(crate) fn scope(self, scope: NodeScope) -> Self;
    pub(crate) fn managed_role(self, role: Option<ManagedNodeRole>) -> Self;
    pub(crate) fn build(self) -> NodeProtocol;
}
```

The builder creates full owned fields; it must not convert missing fields to `TypeExpr::Unknown`, empty parameters, absent consumption/production, or default editor metadata on behalf of a static compatibility type.

- [ ] **Step 1: Record the exact static compatibility inventory**

Run:

```sh
git --no-pager grep -n "StaticNodeProtocol\|StaticNodeCatalogProtocol\|StaticPortSpec\|NodeProtocol::from_static\|protocol_only\|protocol_only_static" -- "src-tauri/src/**/*.rs"
```

Attach the complete match list to the Task 2 review packet.

- [ ] **Step 2: Replace the approving protocol-only test with RED invariant tests**

In `registry/tests.rs`, remove `protocol_only_constructor_registers_without_an_implementation_or_structural_role`. Add:

```rust
#[test]
fn freeze_rejects_node_without_executable_interpretation() { /* expect InvalidNode */ }

#[test]
fn freeze_rejects_node_with_both_leaf_and_structural_interpretations() { /* expect InvalidNode */ }

#[test]
fn leaf_and_structural_nodes_are_the_only_frozen_forms() { /* one of each freezes */ }
```

The no-capability case is fabricated inside the Registry module test using crate-private fields solely to test defensive freeze validation; no public constructor is added.

- [ ] **Step 3: Keep the compiler corruption diagnostic test but remove normal Registry construction**

Rewrite `protocol_only_node_produces_missing_lowering_blocking_diagnostic` so it fabricates a corrupt snapshot through a `#[cfg(test)]` compiler fixture after Registry freeze, then asserts `compiler.lowering.implementation_missing`. The test must not call `RegisteredNode::protocol_only` or prove normal freeze success.

- [ ] **Step 4: Run RED executable-invariant tests serially**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::registry::tests::freeze_rejects_node_without_executable_interpretation -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::registry::tests::freeze_rejects_node_with_both_leaf_and_structural_interpretations -- --exact --test-threads=1
```

Expected: the no-capability case FAILS because `validation.rs` currently accepts `(None, None)`.

- [ ] **Step 5: Enforce the exact freeze XOR**

Change `validate_node` capability matching to:

```rust
match (&node.implementation, node.structural_role) {
    (Some(implementation), None)
        if implementation.capability() == ImplementationKind::CompilerLowering => {}
    (Some(_), None) => return Err(fail("leaf implementation does not provide lowerer capability".into())),
    (None, Some(_)) => {}
    (None, None) => return Err(fail("node has no executable interpretation".into())),
    (Some(_), Some(_)) => return Err(fail(
        "leaf implementation and structural role are mutually exclusive".into(),
    )),
}
```

Do not remove the compiler's later defensive diagnostic.

- [ ] **Step 6: Add the complete owned test protocol builder**

Create `testing/protocol.rs` with explicit `NodeCatalogProtocol`, `NodeInterfaceProtocol`, `ParameterSchema`, `ExecutionSemantics`, `NodeScope`, and `managed_role`. Require callers to pass complete `PortSpec` values including `value_type`, `input_binding`, `consumption`, `production`, `editor`, and `schema`.

- [ ] **Step 7: Migrate Registry/compiler test fixtures first**

Replace `StaticNodeProtocol`/`StaticPortSpec` fixtures in `registry/tests.rs`, `compiler/tests.rs`, and `compiler/tests_dynamic.rs` with `TestProtocolBuilder` plus complete `PortSpec`. Run after each file:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::registry::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::compiler::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::compiler::tests_dynamic -- --test-threads=1
```

- [ ] **Step 8: Migrate document test fixtures**

Replace static compatibility fixtures in `document/tests.rs` and `document/tests/editor_mutation_validation.rs`. Preserve each test's original semantic port type, schema, parameter, binding, consumption/production, and editor contract rather than inserting unknown/default values.

- [ ] **Step 9: Delete static compatibility APIs**

Delete `StaticNodeProtocol`, `StaticNodeCatalogProtocol`, `StaticPortSpec`, `NodeProtocol::from_static`, `NodeCatalogProtocol::from_static`, `RegisteredNode::leaf_static`, `structural_static`, `protocol_only`, and `protocol_only_static`; remove their re-exports/imports.

- [ ] **Step 10: Prove no static/protocol-only symbol remains**

Run:

```sh
git --no-pager grep -n "StaticNodeProtocol\|StaticNodeCatalogProtocol\|StaticPortSpec\|NodeProtocol::from_static\|protocol_only\|protocol_only_static" -- "src-tauri/src/**/*.rs"
```

Expected: no matches and exit status 1.

- [ ] **Step 11: Run GREEN focused Rust tests and gates**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::registry::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::compiler::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::compiler::tests_dynamic -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::document::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

- [ ] **Step 12: Independently review Task 2; do not commit**

Reviewer checks the XOR invariant, public construction surface, defensive compiler diagnostic, every migrated semantic field, and absence of lossy/default compatibility conversion. Resolve all Critical/Important findings and rerun Step 11.

- [ ] **Step 13: Publish Task 2 evidence only after clean review**

Append Task 2 evidence/contracts to the SDD ledger. Update the Phase 1 `TODO.md` row to state executable Registry invariant and static compatibility API removal are independently reviewed; keep it below 100%. Do not commit.

---

### Task 3: Startup Default Locale and Provider Provenance

**Files:**
- Modify: `src-tauri/src/node_system/registry/model.rs`
- Modify: `src-tauri/src/node_system/registry/validation.rs`
- Modify: `src-tauri/src/node_system/registry/mod.rs`
- Modify: `src-tauri/src/node_system/registry/tests.rs`
- Modify: `src-tauri/src/node_system/catalog/builtin.rs`
- Modify: `src-tauri/src/node_system/catalog/localization.rs`
- Modify: `src-tauri/src/node_system/catalog/tests.rs`
- Modify: `src-tauri/src/project/project_store.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `ProviderRegistration`, `BuiltinCatalog`, alias-key inventory, and nominal validator registration.
- Produces:

```rust
pub struct BuiltinNodeSystem {
    pub registry: Arc<NodeRegistry>,
    pub catalog: Arc<BuiltinCatalog>,
}

#[derive(Debug)]
pub enum BuiltinInitializationError {
    Registration(NodeRegistrationError),
    Localization(I18nBundleValidationError),
}

pub fn build_builtin_node_system() -> Result<BuiltinNodeSystem, BuiltinInitializationError>;
pub(crate) fn validate_builtin_bundle(
    provider: ProviderRegistration,
    catalog: BuiltinCatalog,
    alias_keys: BTreeSet<I18nKey>,
) -> Result<BuiltinNodeSystem, BuiltinInitializationError>;

impl NodeRegistry {
    pub fn node_provider(&self, id: &NodeTypeId) -> Option<&ProviderId>;
    pub fn type_provider(&self, id: &TypeId) -> Option<&ProviderId>;
}

impl ProjectStore {
    pub fn try_new() -> Result<Self, BuiltinInitializationError>;
    fn from_builtin(bundle: BuiltinNodeSystem) -> Self;
}

impl ProjectState {
    pub fn try_new() -> Result<Self, BuiltinInitializationError>;
}
```

`NodeRegistry` stores immutable `BTreeMap<NodeTypeId, ProviderId>` and `BTreeMap<TypeId, ProviderId>` indexes. Duplicate registration errors return before a `NodeRegistry` exists and cannot overwrite ownership.

- [ ] **Step 1: Add RED provenance lookup tests**

Add tests with two providers registering distinct nodes/types; freeze in both provider orders and assert exact owners. Add duplicate node/type cases and assert freeze fails without a returned Registry. Add a built-in completeness test:

```rust
for (id, _) in registry.iter() {
    assert_eq!(registry.node_provider(id).map(ProviderId::as_str), Some("yssbi.builtin"));
}
for (id, _) in registry.types().iter() {
    assert_eq!(registry.type_provider(id).map(ProviderId::as_str), Some("yssbi.builtin"));
}
```

- [ ] **Step 2: Run RED provenance tests**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::registry::tests::provider_provenance -- --test-threads=1
```

Expected: FAIL because owner indexes/lookups do not exist.

- [ ] **Step 3: Freeze immutable ownership indexes**

Extend `ValidatedParts` with node/type ownership maps populated at the same point each identity is first inserted. Install them into `NodeRegistry` only after all validation succeeds. Do not derive owners from sorted position, implementation identity, category, or localized metadata.

- [ ] **Step 4: Add RED fallible built-in bundle tests**

Use an injected provider/catalog bundle to test:

```text
missing en-US required key -> BuiltinInitializationError::Localization(MissingDefaultLocale)
alias key stored as Text -> BuiltinInitializationError::Localization(AliasesNotArray)
invalid Registry node -> BuiltinInitializationError::Registration(...)
valid bundle -> one Registry Arc and matching Catalog Arc
```

Assert failure occurs before `ProjectStore`/`ProjectState` construction.

- [ ] **Step 5: Run RED startup tests**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::catalog::tests::builtin_startup -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_store::tests::project_store_requires_validated_builtin_bundle -- --exact --test-threads=1
```

Expected: FAIL because `build_builtin_provider` validates internally and `ProjectStore::default` freezes separately with `expect`.

- [ ] **Step 6: Split assembly from one validation/freeze boundary**

Make built-in assembly return provider, catalog, and alias keys without validating. In `validate_builtin_bundle`, register the provider and nominal validators, freeze the Registry, then call:

```rust
catalog.validate(&registry.catalog_manifest().i18n, &alias_keys)?;
```

Return `BuiltinNodeSystem` only after both operations succeed. Remove `build_builtin_registry` and separate production `build_builtin_provider` call pairs; tests consume `build_builtin_node_system()` or the injectable validator.

- [ ] **Step 7: Make `ProjectStore` consume only the validated bundle**

Implement `ProjectStore::try_new` and private `from_builtin`. Remove Registry/Catalog assembly from `Default`. Update test construction to call `try_new().expect("test built-ins are valid")` only after the fallible boundary; production code must propagate the typed error.

- [ ] **Step 8: Move `ProjectState` creation behind the fallible boundary**

Implement `ProjectState::try_new`/fallible filesystem constructor. Build `ProjectStore` before constructing `project_data`, activation identity, history, or editable stores. Keep `ProjectState::new` only under `#[cfg(test)]` if existing tests need an infallible convenience wrapper around `try_new().expect(...)`.

- [ ] **Step 9: Make Tauri setup fail before managing editable project state**

Remove `.manage(project::ProjectState::new())` from the builder chain. At the start of `.setup`, call `ProjectState::try_new()`, map `BuiltinInitializationError` into the setup error, and only then `app.manage(project_state)`. Do not open, load, or publish a project before this succeeds.

- [ ] **Step 10: Run GREEN startup/provenance suites and gates**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::registry::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::catalog::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_store::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

- [ ] **Step 11: Independently review Task 3; do not commit**

Reviewer checks exact owner lookup, duplicate zero-result behavior, order independence, no localized provenance, one factory boundary, typed error propagation, and no editable state before validation. Resolve all Critical/Important findings and rerun Step 10.

- [ ] **Step 12: Publish Task 3 evidence only after clean review**

Append Task 3 evidence/contracts to the ledger. Update Phase 1 in `TODO.md` with reviewed default-locale startup and provider provenance; keep completion below 100%. Do not commit.

---

### Task 4: Remove the Old Rust Registry and Label Identity

**Files:**
- Create: `src-tauri/src/sci/models/mod.rs`
- Create: `src-tauri/src/sci/models/regression.rs`
- Create: `src-tauri/src/sci/models/panel_did.rs`
- Modify: `src-tauri/src/sci/mod.rs`
- Modify: `src-tauri/src/commands/command_panel_did.rs`
- Modify: `src-tauri/src/execution/source_builder.rs`
- Modify: `src-tauri/src/execution/struct_json.rs`
- Modify: `src-tauri/src/execution/mod.rs`
- Modify: `src-tauri/src/graph/mod.rs`
- Modify: `src-tauri/src/graph/node/mod.rs`
- Modify: `src-tauri/src/schema/node.rs`
- Delete: `src-tauri/src/graph/register/`
- Delete: `src-tauri/src/graph/node/node_definition.rs`
- Delete/migrate: old Registry-dependent files under `src-tauri/src/graph/core/` and `src-tauri/src/graph/infer/`
- Create: `src-tauri/src/node_system/testing/source_audit.rs`
- Modify: `src-tauri/src/node_system/testing/mod.rs`
- Modify: focused command/execution/scientific tests that consume moved types

**Interfaces:**
- Consumes: identity-neutral statistical inputs/results and existing result-source serialization.
- Produces:

```rust
// src-tauri/src/sci/models/panel_did.rs
pub struct ComputeDidFakeGroupRequest {
    pub payload: DidFakeGroupEnginePayload,
    pub n_perm: usize,
    pub rng_seed: u64,
}
pub struct DidPlaceboFakeGroupBlock { /* existing serialized fields unchanged */ }
pub fn compute_fake_group_ri(
    payload: &DidFakeGroupEnginePayload,
    n_perm: usize,
    rng_seed: u64,
) -> Result<DidPlaceboFakeGroupBlock, String>;

// src-tauri/src/sci/models/regression.rs
// Existing serde field names and public fields remain byte-for-byte wire compatible.
pub struct OLSResult { /* moved unchanged */ }
pub struct OLSModel { /* moved unchanged */ }
// Move the currently imported Logit/Probit/Prais and VCE/config types unchanged.
```

Neither module imports `node_system`, `graph::register`, `NodeDefinition`, `PinDefinition`, `PinRole`, `NodeInstanceParams`, localized labels, or node categories.

The production source audit rejects:

```text
mod register / pub mod register under graph
crate::graph::register / graph::register
struct NodeRegistry outside node_system/registry
NodeDefinition / placeholder(
category.join used for node identity
reconcile_node_pins
resolve_dynamic_pins
pin.definition.name used as a dynamic identity key
```

- [ ] **Step 1: Capture the old Registry consumer inventory**

Run and save both complete lists:

```sh
git --no-pager grep -l "crate::graph::register\|graph::register" -- "src-tauri/src/**/*.rs"
git --no-pager grep -l "NodeDefinition\|reconcile_node_pins\|resolve_dynamic_pins\|sync_static_pin_definitions" -- "src-tauri/src/**/*.rs"
```

Classify each file in the review packet as `delete obsolete node authoring/runtime`, `move live identity-neutral science`, or `migrate test to NodeProtocol/GraphDocument`. Every listed file must receive one classification.

- [ ] **Step 2: Add RED identity-neutral science ownership tests**

Add module tests that construct/serialize `OLSModel`, `OLSResult`, and `ComputeDidFakeGroupRequest` through `crate::sci::models`. Add a source assertion that `sci/models/*.rs` contains none of the forbidden imports in the Interfaces block.

- [ ] **Step 3: Add the RED production Registry/label source audit**

Implement recursive Rust source scanning from `env!("CARGO_MANIFEST_DIR")/src`. Strip `#[cfg(test)]` items using the established parser pattern in `project/filesystem/source_audit_tests.rs`; skip conventional test-only files. Scan production `commands/`, `node_system/`, `project/`, `execution/`, `graph/`, and `schema/`, reporting exact `relativePath:line:pattern` offenders.

- [ ] **Step 4: Run RED removal/audit tests serially**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::testing::source_audit::production_has_one_node_registry_and_no_label_identity -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib sci::models -- --test-threads=1
```

Expected: FAIL with old `graph/register`, `NodeDefinition`, placeholder, and display-name dynamic-pin matches.

- [ ] **Step 5: Move Panel DID request/result/engine ownership**

Move existing definitions and `compute_fake_group_ri` from `graph/register/catalog/dataframe/panel_did_engine.rs` into `sci/models/panel_did.rs`. Preserve serde names, deterministic seeded behavior, error strings consumed by `AppError`, and existing focused tests. Change `command_panel_did.rs` to import only `crate::sci::models::panel_did`.

- [ ] **Step 6: Run focused Panel DID tests after the move**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib command_panel_did -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib panel_did -- --test-threads=1
```

- [ ] **Step 7: Move live regression DTO/model ownership**

Move the exact public structs imported by `execution/struct_json.rs` and `source_builder.rs` into `sci/models/regression.rs`. Keep field names/types/serde derives unchanged. Update old scientific implementation files temporarily to import these models from `sci` while their tests are classified; do not add re-exports from `graph::register`.

- [ ] **Step 8: Update result-source consumers and prove wire stability**

Change `execution/source_builder.rs` and `execution/struct_json.rs` imports to `crate::sci::models::regression`. Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib execution::struct_json::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib execution::source_builder -- --test-threads=1
```

The serialized JSON assertions must remain unchanged.

- [ ] **Step 9: Migrate live old-graph tests to current GraphDocument fixtures**

For tests that still prove current behavior, replace `NodeRegistry::new`/`register_builtin_nodes`/`GraphInstance::create_node("Category:Name")` with `build_builtin_node_system()`, stable `NodeTypeId`, and `GraphDocument` creation/mutation/compiler fixtures. Delete tests whose only subject is obsolete old Registry insertion, placeholder replacement, or display-name dynamic-pin reconciliation.

- [ ] **Step 10: Remove obsolete old execution module declarations**

Use `git --no-pager grep` to confirm external consumers, then remove `execution::context` and `execution::engine` module declarations if they are only old `GraphInstance` runtime. Retain `presentation`, `result_source_store`, `runtime_source_invalidation`, `source_builder`, `struct_json`, and any independently consumed data store.

- [ ] **Step 11: Remove old graph authoring/runtime module declarations**

In `graph/mod.rs`, stop compiling `core`, `infer`, and `register` after their live tests/consumers move. Keep `connection`, required identity-neutral `node` data types, `pin`, and `value` only where current project/variable/result-source code still imports them. In `graph/node/mod.rs`, remove `node_definition` and its re-exports.

- [ ] **Step 12: Delete the old Registry and dynamic label identity files**

Delete `graph/register/`, `graph/node/node_definition.rs`, and now-unreferenced old `graph/core`/`graph/infer` files. Remove `schema/node.rs` APIs that consume `NodeDefinition`, or convert genuinely live schema helpers to inputs expressed only in schema/type values.

- [ ] **Step 13: Prove old production symbols and label derivation are absent**

Run:

```sh
git --no-pager grep -n "crate::graph::register\|graph::register\|NodeDefinition\|reconcile_node_pins\|resolve_dynamic_pins\|sync_static_pin_definitions" -- "src-tauri/src/**/*.rs"
git --no-pager grep -n "category.join\|format!(.*category.*name" -- "src-tauri/src/**/*.rs"
```

Expected: no production matches. Test-only source may mention forbidden names only inside the source-audit token construction; it must not compile the old Registry.

- [ ] **Step 14: Run GREEN source audit, focused consumers, and Rust gates**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::testing::source_audit -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib sci::models -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib command_panel_did -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib execution::struct_json::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

- [ ] **Step 15: Independently review Task 4; do not commit**

Reviewer checks every inventory classification, scientific dependency neutrality, serde/API preservation, deletion safety, production module declarations, one Registry authority, no placeholder path, and no label-based dynamic identity. Resolve all Critical/Important findings and rerun Step 14.

- [ ] **Step 16: Publish Task 4 evidence only after clean review**

Append the complete classification table, deletions, moved contracts, and validation output to the ledger. Update Phase 1 in `TODO.md` to state old Rust Registry/label identity removal is independently reviewed; keep completion below 100%. Do not commit.

---

### Task 5: Rust↔TypeScript Golden Contracts and Fingerprint Matrix

**Files:**
- Create: `src/tests/fixtures/node-system-contracts/semantic-protocol.json`
- Create: `src/tests/fixtures/node-system-contracts/i18n-inventory.json`
- Create: `src/tests/fixtures/node-system-contracts/localized-catalog.json`
- Create: `src/tests/fixtures/node-system-contracts/editor-projection.json`
- Create: `src/tests/fixtures/node-system-contracts/fingerprint-wire.json`
- Create: `src-tauri/src/node_system/testing/contracts.rs`
- Modify: `src-tauri/src/node_system/testing/mod.rs`
- Modify: `src-tauri/src/node_system/testing/tests.rs`
- Modify: `src-tauri/src/node_system/registry/tests.rs`
- Modify: `src-tauri/src/node_system/analysis/projection.rs`
- Modify: `src/shared/types/dto/editorProjection.ts`
- Modify: `src/features/domain/editorProjection/validateProjection.ts`
- Modify: `src/services/nodeSystem/graphProjectionService.ts`
- Create: `src/services/nodeSystem/nodeSystemGoldenContracts.test.ts`
- Modify: `src/services/nodeSystem/graphProjectionService.test.ts`
- Modify: `src/features/domain/editorProjection/editorProjection.test.ts`
- Modify: Catalog DTO/service tests if fixture-driven strict parsing exposes drift

**Interfaces:**
- Consumes: validated built-in bundle from Task 3, stable identities from Task 1, and one Registry authority from Task 4.
- Produces the exact update switch:

```text
YSSBI_UPDATE_NODE_CONTRACT_FIXTURES=1
```

Ordinary tests never write fixtures. With the switch set, the Rust contract test writes all five files deterministically and then compares parsed JSON values.

- All IPC Registry fingerprints use lowercase 64-character SHA-256 hex strings. Internal `RegistryFingerprint([u8; 32])` remains unchanged.
- `ProjectionBasisDto` becomes:

```ts
export interface ProjectionBasisDto {
  graphPath: string;
  graphRevision: number;
  registryFingerprint: string;
  resourceVersions: Record<string, string>;
}

export function isEditorGraphProjectionDto(value: unknown): value is EditorGraphProjectionDto;
export function parseEditorGraphProjectionDto(value: unknown): EditorGraphProjectionDto;
```

- `GraphProjectionService.loadGraph` and `hydrateGraph` invoke as `unknown`, call `parseEditorGraphProjectionDto`, and throw `Invalid editor graph projection response` for malformed wire data.
- `fingerprint-wire.json` documents/tests this matrix:

```json
{
  "format": "yssbi.registry-fingerprint-wire.v1",
  "catalog": "<64 lowercase hex>",
  "editorProjection": "<same 64 lowercase hex>",
  "runEvent": "<same 64 lowercase hex>",
  "trace": "<same 64 lowercase hex>"
}
```

- [ ] **Step 1: Add RED table-driven duplicate identity coverage**

In `registry/tests.rs`, add one table covering duplicate provider, node, type, type constructor, type class, category, i18n key, interface resolver, schema resolver, and nominal validator. Each case asserts the exact `RegistryValidationError` variant and proves no frozen Registry is returned.

- [ ] **Step 2: Add RED fingerprint sensitivity/insensitivity matrix**

Add table-driven cases proving fingerprint changes for lowerer implementation identity, structural role, type definition/classes, type constructor arity, interface resolver inventory, schema resolver inventory, and nominal validator identity/version. Prove it does not change for provider order, title/description/aliases keys or localized text, icon/style/hidden display metadata, category order/arrangement, and selected locale. Assert canonical JSON contains no `0x`, pointer debug text, or process-local address.

- [ ] **Step 3: Run RED Registry matrix tests**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::registry::tests::duplicate_global_identity_matrix -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::registry::tests::registry_fingerprint_matrix -- --exact --test-threads=1
```

Expected: missing matrix cases fail until canonical inputs/provenance are complete.

- [ ] **Step 4: Implement missing canonical Registry inputs**

Extend `canonical_registry` only for semantic fields named in Step 2. Sort every provider/type/node/resolver/class collection by stable ID. Never hash localization values, category display arrangement, pointers, `Arc` debug output, or map insertion order.

- [ ] **Step 5: Add RED Rust fixture harness with exact paths**

In `testing/contracts.rs`, set:

```rust
const FIXTURE_ROOT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../src/tests/fixtures/node-system-contracts"
);
const UPDATE_ENV: &str = "YSSBI_UPDATE_NODE_CONTRACT_FIXTURES";
```

Generate parsed `serde_json::Value` for all five fixtures. If a fixture differs and the switch is absent, assert equality with the fixture path in the message. If the switch equals `1`, create the directory and write pretty JSON with one trailing newline.

- [ ] **Step 6: Define deterministic fixture contents**

Generate:

```text
semantic-protocol.json: canonical_semantic_protocol_snapshot for the built-in Registry
i18n-inventory.json: format, defaultLocale, required keys, alias keys, Registry fingerprint
localized-catalog.json: en-US categories plus one static, one parameterizedStatic, and function/variable/database resourceBound descriptor
editor-projection.json: one stable bool constant projection with display, declared port, parameter editor, capabilities, diagnostics array, and hex fingerprint
fingerprint-wire.json: the four purpose-specific encodings from the Interfaces block
```

Use fixed UUIDs, graph path `events/contract.yssbi-event`, revisions, resource paths, and resource ordering. Do not use current time, random UUIDs, filesystem order, or hash-map iteration order.

- [ ] **Step 7: Run RED Rust contract test**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::testing::contracts::checked_in_node_system_contracts_match_rust -- --exact --test-threads=1
```

Expected: FAIL listing the first absent fixture.

- [ ] **Step 8: Generate fixtures explicitly once**

Run:

```sh
YSSBI_UPDATE_NODE_CONTRACT_FIXTURES=1 CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::testing::contracts::checked_in_node_system_contracts_match_rust -- --exact --test-threads=1
```

Then rerun the same command without `YSSBI_UPDATE_NODE_CONTRACT_FIXTURES`; it must PASS without changing file timestamps/content.

- [ ] **Step 9: Canonicalize editor projection fingerprint wire encoding**

Add a Serde serializer on `ProjectionBasis.registry_fingerprint` that emits `to_hex()`. Change TypeScript `ProjectionBasisDto.registryFingerprint` from `number[]` to `string`. Keep internal Rust equality/comparison on `RegistryFingerprint`, not strings.

- [ ] **Step 10: Add complete strict editor projection guards**

Implement exact-key guards for the root, basis, node, display, capability, port/address variants, connection, input binding, type/schema summaries, parameter editors/configuration variants, diagnostics/location variants, and all enums. Require `registryFingerprint` to match `/^[0-9a-f]{64}$/`, revisions to be non-negative safe integers, and reject unknown/missing keys before `validateEditorGraphProjection` checks graph/revision/ownership coherence.

- [ ] **Step 11: Make projection services parse unknown wire values**

Change both GraphProjection service methods to await `invoke<unknown>`, call `parseEditorGraphProjectionDto`, and return only validated DTOs. Preserve command names/arguments.

- [ ] **Step 12: Add RED TypeScript golden and mutation tests**

`nodeSystemGoldenContracts.test.ts` imports all five JSON fixtures with JSON module support already provided by Vite/Vitest. Assert real `isLocalizedCatalogDto`, `isNodeCreationDescriptorDto`, and `parseEditorGraphProjectionDto` accept authoritative fixtures. For each Catalog/projection descriptor variant, clone and test: one unknown key, one missing required key, wrong discriminant, and wrong fingerprint encoding (`number[]`, uppercase hex, 63 chars).

- [ ] **Step 13: Run GREEN Rust/TypeScript contract suites**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::testing::contracts -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::registry::tests -- --test-threads=1
pnpm test src/services/nodeSystem/nodeSystemGoldenContracts.test.ts src/services/nodeSystem/catalogService.test.ts src/services/nodeSystem/graphProjectionService.test.ts src/features/domain/editorProjection/editorProjection.test.ts
pnpm typecheck
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

- [ ] **Step 14: Verify ordinary tests are read-only**

Run `git --no-pager diff --` for the five fixtures, rerun the Rust contract test without the update switch, and rerun the diff. Expected: identical fixture diff before/after the ordinary test.

- [ ] **Step 15: Independently review Task 5; do not commit**

Reviewer checks Rust authority, deterministic generation, read-only default, strict real parsers, malformed cases, all five fixture purposes, explicit fingerprint encoding, full duplicate matrix, semantic/display hash separation, and no code-generation dependency. Resolve all Critical/Important findings and rerun Steps 13-14.

- [ ] **Step 16: Publish Task 5 evidence only after clean review**

Append fixture hashes, commands, matrix coverage, and parser contracts to the ledger. Update Phase 1 in `TODO.md` to state golden contracts/fingerprint matrix are independently reviewed; keep completion below 100%. Do not commit.

---

### Task 6: Final Whole-Slice Acceptance

**Files:**
- Review: every path changed by Tasks 1-5
- Modify after clean review and verification only: `.superpowers/sdd/2026-08-04-phase1-registry-identity-closure/progress.md`
- Modify after clean review and verification only: `TODO.md`
- Modify for execution tracking only: `docs/superpowers/plans/2026-08-04-phase1-registry-identity-closure.md`

**Interfaces:**
- Consumes: five independently reviewed deliverables and their ledger evidence.
- Produces: one stable-identity frontend, one executable/provenance-preserving Registry, validated startup, no production old Registry/label identity, five passing Rust↔TS golden fixtures, and final Phase 1 acceptance evidence.
- No production implementation is introduced in this task. Any Critical/Important finding reopens the owning earlier task, which must repeat its focused GREEN commands and independent review before Task 6 resumes.

- [ ] **Step 1: Confirm branch and preserve dirty-work boundaries**

Run:

```sh
git --no-optional-locks status --short --branch
git --no-pager diff --stat
git --no-pager diff --check
```

Expected branch: `shadcn`. Compare the status with Task 1's captured baseline and identify only planned additions/removals plus unrelated preserved work.

- [ ] **Step 2: Run frontend stable-identity acceptance files**

Run:

```sh
pnpm test src/features/domain/nodeCatalog/identity.test.ts src/features/domain/graphDiagnostics/callFunctionDiagnostics.test.ts src/features/application/editor/cascadeGraphPathReferences.test.ts src/features/application/editorMutation/projectPublicationMovePlan.test.ts src/views/EditorView/Nodes/DefaultNodeLayout.test.tsx src/features/application/editor/canvasDrop/variableDrop.test.ts src/features/application/editor/canvasDrop/spawnFromTemplate.test.ts src/shared/utils/pinCompatibility.test.ts src/services/nodeSystem/nodeIdentityArchitectureContract.test.ts
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 3: Run focused Registry/protocol/startup suites serially**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::registry::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::compiler::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::compiler::tests_dynamic -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::catalog::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_store::tests -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 4: Run old-Registry removal and science consumer suites serially**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::testing::source_audit -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib sci::models -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib command_panel_did -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib execution::struct_json::tests -- --test-threads=1
```

Expected: PASS.

- [ ] **Step 5: Run golden contract/fingerprint acceptance**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::testing::contracts -- --test-threads=1
pnpm test src/services/nodeSystem/nodeSystemGoldenContracts.test.ts src/services/nodeSystem/catalogService.test.ts src/services/nodeSystem/graphProjectionService.test.ts src/features/domain/editorProjection/editorProjection.test.ts
```

Expected: PASS and no fixture rewrite.

- [ ] **Step 6: Run static symbol audits directly**

Run:

```sh
git --no-pager grep -n "StaticNodeProtocol\|StaticNodeCatalogProtocol\|StaticPortSpec\|NodeProtocol::from_static\|protocol_only\|protocol_only_static" -- "src-tauri/src/**/*.rs"
git --no-pager grep -n "crate::graph::register\|graph::register\|NodeDefinition\|reconcile_node_pins\|resolve_dynamic_pins" -- "src-tauri/src/**/*.rs"
git --no-pager grep -n "Functions:Call Function\|Variables:Get Variable\|Variables:Set Variable\|Data:Get DataFrame\|resolveEffectiveDefinition" -- "src/**/*.ts" "src/**/*.tsx"
```

Expected: no production matches; only audit token construction or explicitly test-only negative assertions may appear. Any production match reopens its owning task.

- [ ] **Step 7: Run Rust static gates**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Expected: PASS.

- [ ] **Step 8: Dispatch final whole-slice spec review**

Give the reviewer the approved design, this plan, all Task 1-5 ledger entries, complete planned diff, source-audit outputs, and fixture files. Reviewer must verify every design goal/non-goal, exact task order, no compatibility paths, startup-before-editability, ownership/fingerprint determinism, and behavior preservation.

- [ ] **Step 9: Dispatch final whole-slice quality review**

A separate reviewer checks architecture boundaries, error handling, lock/startup behavior, test quality, deletion safety, strict parser completeness, fixture update safety, and unrelated dirty-work preservation. Record findings by severity and owning task.

- [ ] **Step 10: Resolve review findings through the owning task**

For each Critical/Important finding, return to its Task 1-5 RED/GREEN loop, add a focused regression test, rerun that task's commands, and obtain a new clean independent review. Resume Task 6 only when both whole-slice reviews report no Critical or Important findings.

- [ ] **Step 11: Run the required fresh cross-layer verification**

Run exactly:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm verify
```

Expected: frontend typecheck/tests, Rust fmt/check/tests, scientific tests, and diff check all PASS. Do not substitute earlier outputs.

- [ ] **Step 12: Run final workspace hygiene checks**

Run:

```sh
git --no-optional-locks status --short --branch
git --no-pager diff --check
git --no-pager diff -- docs/superpowers/plans/2026-08-04-phase1-registry-identity-closure.md .superpowers/sdd/2026-08-04-phase1-registry-identity-closure/progress.md TODO.md
```

Confirm branch remains `shadcn`, no worktree/branch/tag/commit was created, fixtures were not rewritten by ordinary tests, and unrelated dirty files remain intact.

- [ ] **Step 13: Publish final acceptance only after clean reviews and Step 11 PASS**

Append whole-slice reviewer results, exact `pnpm verify` output summary, fixture hashes, and final hygiene result to the SDD ledger. Set Phase 1 to **100%** in `TODO.md` with a concise statement that stable identity, executable Registry invariant, startup locale/provenance, old Registry removal, golden contracts, and final whole-slice verification are complete. Do not commit or tag.

---

## Plan Self-Review Checklist

- [x] Spec coverage: every requirement in the approved design maps to one of Tasks 1-6; non-goals remain excluded.
- [x] Required order: frontend identity → executable Registry/static removal → startup locale/provenance → old Rust Registry removal → golden/fingerprint contracts → whole-slice acceptance.
- [x] RED-GREEN independence: each task has focused failing tests, minimal production changes, passing commands, and its own clean review gate.
- [x] No commits: every task explicitly forbids commit/tag/branch/worktree creation.
- [x] Publication discipline: ledger and `TODO.md` updates occur only after clean independent review; 100% occurs only after final clean reviews and fresh `pnpm verify`.
- [x] Type consistency: stable ID constants, `BuiltinNodeSystem`, provenance lookups, owned test protocol builder, fingerprint strings, and parser signatures are consistent across tasks.
- [x] Placeholder scan: all files, symbols, fixture names, test names, commands, expected failures, and acceptance conditions are concrete.
