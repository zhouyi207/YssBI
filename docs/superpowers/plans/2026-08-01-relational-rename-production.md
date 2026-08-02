# Relational Rename Production Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a built-in `yssbi.dataframe.rename` node and prove Source → Rename → Limit executes as one production relational island with strict schema/runtime semantics.

**Architecture:** Rust owns the protocol, scalar rename parameters, Schema Algebra evaluation, relational lowering, backend validation, and execution. The node lowers to the existing `RelationalOperator::Rename`; no DataSeries expression lineage, frontend inference, or second planner is introduced.

**Tech Stack:** Rust Node Protocol, Schema Algebra, GraphCompiler, relational plan IR, ProductionRelationalBackend, ProjectState.

## Global Constraints

- Work directly on `shadcn`; no worktree, branch, or commit.
- Preserve all existing Static Catalog, observability, execution IPC, `.gitignore`, and `TODO.md` changes.
- Rust remains authoritative for protocol, parameters, schema, lowering, execution, diagnostics, and stable IDs.
- Use real built-in protocols and `ProjectState::execute_graph` for final production coverage.
- Do not reinterpret Filter, Decompose, Series Select, Combine, or Union.
- Rename exactly one column using required `from` and `to` string parameters.
- `from == to` is a valid deterministic no-op in schema analysis and runtime.
- The static palette excludes Rename while required parameters have no defaults.
- Use focused serial Rust tests only; do not rerun the known-red complete Rust suite.
- Update only the `TODO.md` `## node_architecture 进度` table after each completed task.

---

## File Structure

- Modify `src-tauri/src/node_system/catalog/dataframe/families.rs` to register the non-legacy Rename built-in.
- Modify `src-tauri/src/node_system/catalog/dataframe/mod.rs` for protocol, localization, schema expression, and `RenameLowerer`.
- Modify `src-tauri/src/node_system/catalog/dataframe/tests.rs` for frozen protocol/lowering contracts.
- Modify `src-tauri/src/node_system/protocol/types.rs` only if parameter-driven rename is not expressible by the existing `SchemaExpr::Rename` mapping contract.
- Modify `src-tauri/src/node_system/compiler/schema_analysis.rs` for parameter-driven strict rename validation.
- Modify `src-tauri/src/node_system/runtime/production_relational.rs` for strict Rename evaluation.
- Modify `src-tauri/src/project/production_tests.rs` for the authoritative Source → Rename → Limit production graph.

### Task 1: Freeze the built-in Rename protocol and lowering

**Interfaces:**

- Produces node ID `yssbi.dataframe.rename`.
- Declared ports: input `source`, output `result`.
- Required string parameters: `from`, `to`.
- Lowered operators: one local `Input` followed by one `Rename` mapping.

- [ ] **Step 1: Add failing Catalog tests** in `catalog/dataframe/tests.rs` asserting exact stable ID, category, declared ports, streaming contracts, required parameter keys/types, complete en-US/zh-CN localization, static Catalog exclusion, and exact `Input + Rename` fragment.
- [ ] **Step 2: Run the exact new protocol/lowering tests serially** and confirm failure because the node is absent.

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::catalog::dataframe::tests::rename_dataframe --test-threads=1
```

- [ ] **Step 3: Add a non-legacy Rename specification** without changing `LEGACY_NODE_IDS` or the migrated legacy count.
- [ ] **Step 4: Implement the owned protocol** with `source`/`result`, required `from`/`to`, streaming contracts, Schema Rename declaration, localization, and a `RenameLowerer` that emits exact existing IR.
- [ ] **Step 5: Ensure static descriptor eligibility excludes Rename** because both required parameters lack defaults; do not add frontend special cases.
- [ ] **Step 6: Re-run the exact tests and related Catalog eligibility tests.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::catalog::dataframe::tests::rename_dataframe --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::catalog::tests::static_catalog_excludes_managed_and_resource_required_descriptors --exact --test-threads=1
```

### Task 2: Enforce strict parameter-driven Schema Rename semantics

**Interfaces:**

- Input: analyzed source `SchemaState` plus validated `from`/`to` parameter values.
- Output: source schema with exactly one field key renamed; all field types preserved.
- Same-name rename returns the unchanged schema.

- [ ] **Step 1: Add failing schema-analysis tests** for valid rename, missing source, destination collision, blank name, whitespace-padded name, invalid parameter type, unknown input schema, and same-name no-op.
- [ ] **Step 2: Run the focused schema tests** and confirm the existing generic Rename path cannot satisfy the parameter contract.

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::compiler::schema_analysis::tests::rename_dataframe --test-threads=1
```

- [ ] **Step 3: Extend Schema Algebra minimally** so Rename reads the exact `from` and `to` scalar parameter keys. Do not add a generic object/mapping editor or frontend interpretation.
- [ ] **Step 4: Emit existing structured diagnostics** at the Rename node/parameter for blank names, missing source, conflict, and invalid type; return no semantic plan for blocking errors.
- [ ] **Step 5: Preserve field types and deterministic ordering**, with `from == to` returning the unchanged schema.
- [ ] **Step 6: Re-run schema tests and the complete focused compiler filter.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::compiler::schema_analysis::tests::rename_dataframe --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::compiler::tests -- --test-threads=1
```

### Task 3: Harden production relational Rename execution

**Interfaces:**

- Consumes `RelationalOperator::Rename { input, columns }`.
- Missing source and destination collision return `RelationalError`.
- Same-name entry is a no-op.

- [ ] **Step 1: Add failing backend tests** for value preservation, old-name removal, new-name insertion, untouched columns, row-count preservation, missing source rejection, destination collision rejection, and same-name no-op.
- [ ] **Step 2: Run the focused backend Rename tests** and confirm current silent-ignore/overwrite behavior fails them.

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::production_relational::tests::rename --test-threads=1
```

- [ ] **Step 3: Implement strict Rename evaluation** before mutating the object: validate the complete mapping against the source field set, reject duplicate/conflicting destinations, then apply atomically.
- [ ] **Step 4: Preserve every column vector and row count**; never silently drop, ignore, or overwrite unrelated fields.
- [ ] **Step 5: Re-run focused Rename tests and the production relational backend filter.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::production_relational::tests::rename --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::runtime::production_relational::tests -- --test-threads=1
```

### Task 4: Prove Source → Rename → Limit through ProjectState

**Interfaces:**

- Uses only built-in `source.get`, `dataframe.rename`, and `dataframe.limit` protocols.
- Executes only through `ProjectState::execute_graph`.

- [ ] **Step 1: Add a failing authoritative production test** that creates a temporary project/database fixture, builds the normalized GraphDocument with exact stable declared ports and Rename parameters, and executes it through `ProjectState::execute_graph`.
- [ ] **Step 2: Assert one relational subplan, one backend invocation, zero bridges, exact Source → Rename → Limit operator/root order, and suppression of Limit source pushdown because the exposed Rename root requires the complete source.**
- [ ] **Step 3: Assert both outputs:** the exposed Rename root contains every renamed source row, while the final Limit root contains exactly the requested limited rows; both exclude the old name and preserve untouched columns.
- [ ] **Step 4: Add reversed document insertion coverage** and compare normalized plans/results while ignoring run/compile identity fields that are intentionally unique.
- [ ] **Step 5: Run the exact production tests and focused relational/compiler filters.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::project_execute_graph_runs_builtin_dataframe_source_rename_limit --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::project_execute_graph_source_rename_limit_is_insertion_order_independent --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::compiler::tests -- --test-threads=1
```

### Task 5: Slice verification and progress publication

- [ ] **Step 1: Run all Rename-focused Catalog, schema, backend, compiler, and ProjectState tests serially.**
- [ ] **Step 2: Run the relevant broader Catalog/compiler/plan/production-relational filters.**
- [ ] **Step 3: Run required gates.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

- [ ] **Step 4: Do not run the complete Rust suite.** Record the existing known-red baseline and only the fresh focused results.
- [ ] **Step 5: Confirm `TODO.md` reflects verified Phase 7 progress** and that no unrelated table content was changed.
