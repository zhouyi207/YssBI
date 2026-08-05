# Phase 1 Registry and Stable Identity Closure Design

## Status

Approved on 2026-08-04.

This design closes Phase 1 of `docs/plan/node-architecture.md`. It removes the remaining compatibility identities and makes the Rust Registry the only production authority for node protocol, implementation capability, provider ownership, localization inventory, and cross-language contracts.

## Goals

1. Every frozen Registry node has exactly one executable interpretation: a lowerer-backed leaf or a compiler-recognized structural role.
2. Rust and frontend production code identify nodes only by stable `NodeTypeId` values.
3. The old label/category-based Rust Registry and dynamic-pin identity system are removed rather than retained as a second compatibility path.
4. Built-in startup fails before editable state exists when Registry or default-locale contracts are incomplete.
5. Frozen Registry data preserves provider provenance.
6. Rust-generated golden fixtures prove that frontend DTO parsers consume the authoritative wire contract.
7. Phase 1 reaches 100% only after independent task reviews, a whole-slice review, focused checks, and `pnpm verify`.

## Non-goals

- Changing graph execution semantics, scheduling, relational behavior, or structured control.
- Adding plugin loading, dynamic provider unloading, or provider trust policy.
- Introducing a new UI component library.
- Adding a code-generation dependency solely for TypeScript DTOs.
- Preserving deprecated `category:name` identities through aliases or migration shims.
- Refactoring scientific algorithms that do not depend on the old node identity model.

## Stable identity contract

Production node identities are Rust-defined stable IDs. At minimum, resource nodes use:

```text
yssbi.project.function.call
yssbi.project.variable.get
yssbi.project.variable.set
```

Database node IDs use the exact stable IDs registered by the built-in Registry. Frontend code imports stable constants from one domain module or consumes IDs from Rust descriptors; it never reconstructs them from display titles, categories, localization keys, or resource paths.

The following strings are forbidden in frontend production identity comparisons:

```text
Functions:Call Function
Variables:Get Variable
Variables:Set Variable
Data:Get DataFrame
```

They may appear only in an explicitly scoped historical migration fixture if a persisted legacy-format reader still requires them. No current-schema authoring, projection, mutation, layout, diagnostics, rename, move, or drag/drop path may emit or match them.

A source-audit test scans frontend production files and fails if forbidden identities or deleted compatibility resolvers return.

## Registry executable-state invariant

A `RegisteredNode` must satisfy exactly one of these forms:

```text
Leaf:       implementation = Some(lowerer), structural_role = None
Structural: implementation = None,          structural_role = Some(role)
```

The following forms are invalid at Registry freeze:

```text
implementation = None,          structural_role = None
implementation = Some(lowerer), structural_role = Some(role)
```

The invalid no-capability state is not retained for editor-only or future nodes. A node is exposed in Catalog only after its real lowerer or structural role exists. This prevents Registry success followed by a later `leaf_without_operation` compiler failure.

`RegisteredNode::protocol_only`, `RegisteredNode::protocol_only_static`, and tests that approve no-capability registration are removed. The compiler keeps a defensive diagnostic for corrupt or manually fabricated snapshots, but normal Registry construction cannot produce that state.

## Static protocol compatibility removal

`StaticNodeProtocol`, `StaticPortSpec`, and static registration constructors currently discard type, schema, parameter, consumption, production, and editor information. They are removed rather than expanded.

Tests and helper providers construct the full owned `NodeProtocol`. This follows the project 0.x policy of deleting deprecated shims instead of maintaining parallel compatibility models.

No production or test provider may register a protocol whose missing semantic fields are silently replaced with `TypeExpr::Unknown`, empty parameters, or default editor behavior by a compatibility conversion.

## Provider provenance

The frozen `NodeRegistry` retains immutable ownership indexes:

```text
NodeTypeId -> ProviderId
TypeId     -> ProviderId
```

If type constructors, type classes, interface resolvers, schema resolvers, or nominal validators require owner queries for validation or diagnostics, their ownership is retained through the same frozen provenance model rather than inferred from registration order.

Required behavior:

- lookup returns the exact registering provider;
- duplicate identities remain registration errors and never overwrite provenance;
- provider registration order does not change the frozen Registry fingerprint or ownership result;
- provenance is semantic Registry data but does not expose localized display metadata;
- built-in single-owner tests cover every registered node and type.

## Default-locale startup validation

Built-in initialization becomes one validated factory boundary. It constructs:

1. the built-in `ProviderRegistration`;
2. the built-in localization catalog;
3. the frozen `NodeRegistry`;
4. validation of the default locale against the Registry i18n inventory and alias-array requirements.

`ProjectStore` is created only from this validated bundle. Missing default-locale keys, malformed aliases, Registry validation failures, or disagreement between provider and catalog abort initialization before editable project state is available.

Tests use an injectable factory or validation helper to prove missing default-locale data fails. Production startup must not rely on a test-only audit.

## Legacy Rust Registry removal

The old `graph::register::NodeRegistry`, label-derived `NodeDefinition` identity builder, and dynamic-pin matching by display name are removed from production compilation.

Removal process:

1. Audit all consumers of the old Registry, `NodeDefinition`, and dynamic-pin identity helpers.
2. Classify each consumer as:
   - obsolete node authoring/runtime code to delete;
   - pure scientific/domain logic to move behind an identity-neutral interface;
   - test fixture to migrate to full `NodeProtocol` or current `GraphDocument`.
3. Migrate required identity-neutral code without importing `node_system` into scientific modules.
4. Delete the old Registry and label-derived identity construction.
5. Add a Rust source-audit test covering production project, command, catalog, compiler, runtime, and graph modules.

The audit rejects production references that recreate a second node Registry, derive node identity from category/name, or match dynamic ports by localized/display labels.

A temporary `cfg(test)` isolation is acceptable only within the task that removes the final test dependency. Phase 1 cannot be marked complete while the old Registry remains compiled in production.

## Frontend behavior migration

Stable-ID migration covers every current production use, including:

- Call Function resource-title projection;
- missing function target diagnostics;
- graph rename/cascade caller detection;
- publication move planning;
- variable get/set layout and modifiers;
- database resource layout;
- function/variable/database canvas-drop creation;
- Catalog descriptor classification.

Resource authoring continues through exact Rust-issued `NodeCreationDescriptor` values. Frontend code does not synthesize resource node parameters or call `invoke` from views. Resource paths remain opaque.

Tests begin with Rust-shaped projections or shared Rust-generated fixtures containing stable IDs. They prove the previous behavior still occurs under stable identity and that legacy strings no longer match production paths.

## Cross-language golden contract

The repository checks in Rust-generated JSON fixtures for:

1. canonical semantic protocol snapshot;
2. i18n inventory;
3. localized Catalog descriptor examples, including `ParameterizedStatic` and resource descriptors;
4. editor projection DTO examples;
5. Registry fingerprint wire representation used by each purpose-specific DTO.

Rust tests regenerate values in memory and compare them structurally with checked-in fixtures. Fixture updates require an explicit test-only environment switch; ordinary tests are read-only.

TypeScript tests import the same fixtures and pass them through the real strict DTO guards and service parsers. Unknown keys, missing keys, wrong descriptor variants, and wrong fingerprint encodings fail.

Purpose-specific fingerprint encodings may differ only when documented and tested. The contract must not leave an accidental `number[]` versus string disagreement without an explicit boundary rule.

This design uses contract fixtures rather than introducing a DTO generation package. If future DTO volume makes fixtures insufficient, code generation is a separate architecture decision.

## Fingerprint and duplicate validation matrix

Focused table-driven tests close the remaining Registry matrix:

- every duplicate global identity variant is rejected without mutation;
- lowerer implementation identity changes Registry fingerprint;
- structural role changes Registry fingerprint;
- type definition or class membership changes Registry fingerprint;
- type constructor arity changes Registry fingerprint;
- interface/schema resolver inventories change Registry fingerprint;
- provider registration order does not change fingerprint;
- no fingerprint input contains pointer values or process-local addresses;
- display title, description, aliases, category arrangement, and locale do not change semantic fingerprints.

## Error handling

- Registry and localization failures are typed startup errors.
- Duplicate or invalid registrations never partially mutate the frozen result.
- Public errors do not expose arbitrary provider internals or localized text as identity.
- Contract fixture mismatches report the exact fixture and structural difference through normal test output.
- Legacy identity detection fails tests at build time; it is not silently translated at runtime.

## Testing strategy

Every implementation task follows RED-GREEN and receives an independent spec/quality review.

Focused validation includes:

- Rust protocol and Registry suites;
- built-in Catalog and startup validation suites;
- frontend node identity, diagnostics, rename/move, layout, drop, service, and DTO contract suites;
- Rust source audits and TypeScript source audits;
- `pnpm typecheck`;
- `pnpm rust:check`;
- `pnpm rust:fmt:check`;
- `git --no-pager diff --check`.

Because the completed slice spans Rust and frontend, final delivery runs:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 pnpm verify
```

Phase 1 reaches 100% only after the final whole-slice reviewer reports no Critical or Important findings.

## Delivery and progress publication

Work remains directly on branch `shadcn`. No worktree, branch, commit, or tag is created. Unrelated dirty changes are preserved.

After each independently reviewed task, append evidence and contracts to this plan's SDD ledger and update `TODO.md` under `## node_architecture 进度`. Intermediate percentages describe remaining reviewed work; 100% is reserved for clean final review plus fresh controller verification.
