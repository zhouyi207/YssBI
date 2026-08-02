# Relational Rename Production Slice Design

**Date:** 2026-08-01

## Status and scope

This slice extends the production relational island beyond DataFrame Source and
Limit by adding one explicit DataFrame-returning Rename node. It does not reuse
or reinterpret existing Filter, Decompose, Series Select, Combine, or Union
nodes because their public semantics do not match the similarly named
relational IR operators.

The production graph for this slice is:

```text
Get DataFrame → Rename DataFrame → Limit DataFrame
```

The acceptance target is one relational island, one backend invocation, and no
materialization bridge.

## Constraints

- Rust remains authoritative for protocol, parameters, schema, lowering,
  execution, diagnostics, and stable IDs.
- Work directly on `shadcn`; do not create a worktree or commit.
- Use real built-in protocols and `ProjectState::execute_graph` for final
  production coverage.
- Do not reinterpret `yssbi.dataframe.combine` as relational Union.
- Do not change the mask-based public contract of `yssbi.dataframe.filter`.
- Do not introduce DataSeries expression lineage or dynamic-column materialization.
- Do not hold project/global locks during I/O, compilation, or execution.
- Use focused serial Rust tests; do not rerun the complete Rust suite in this
  slice because its known failing baseline is already recorded.
- Update the `TODO.md` `## node_architecture 进度` table after each completed
  implementation task.

## Built-in node protocol

Add one built-in node:

```text
NodeTypeId: yssbi.dataframe.rename
```

Interface:

- declared input `source: tabular.dataframe`;
- declared output `result: tabular.dataframe`;
- streaming-compatible input consumption and output production.

Parameters:

- `from: core.string`, required;
- `to: core.string`, required.

The first production contract renames exactly one column per node. A structured
mapping parameter or multi-row editor is intentionally deferred.

The node is a normal static Catalog descriptor only when the creation contract
can construct a valid document without frontend-supplied required values. Since
`from` and `to` are required and have no defaults, the generic static palette
must exclude it until a resource/contextual descriptor supplies parameters or a
later protocol design provides valid defaults. Tests must not weaken the static
Catalog eligibility rule merely to expose this node in the current palette.

The node receives complete en-US and zh-CN catalog, parameter, and port
localization entries. Its stable technical identity never depends on localized
text.

## Schema semantics

The node's schema expression renames one field of the input DataFrame.
Compilation validates:

1. `from` and `to` are non-empty strings;
2. neither value has leading or trailing whitespace;
3. the input schema contains `from`;
4. `to` does not collide with an untouched input column;
5. renaming a column to the same name is a valid deterministic no-op;
6. all other fields and types are preserved.

The same-name no-op policy is shared by schema analysis and runtime validation
and is frozen in focused tests.

When the input schema is unavailable, analysis remains available with a
structured diagnostic and no executable plan. Diagnostics identify the node
and relevant parameter rather than exposing raw backend errors.

## Lowering

The node lowers to the existing relational IR without adding a new operator:

```text
Input(source)
→ Rename {
    input: <input operator>,
    columns: [{ from, to }]
  }
```

The lowerer reads only validated compiled parameters and stable local port
bindings. It does not inspect labels, localized text, document insertion order,
or frontend-derived schema.

When connected after `yssbi.dataframe.source.get` and before
`yssbi.dataframe.limit`, relational planning must merge all fragments into one
maximal island. No intermediate DataFrame is materialized and no bridge is
created.

## Runtime semantics

The production relational backend must apply Rename strictly:

- preserve every source column value;
- remove the old field name after a successful rename;
- insert the new field name with the same values;
- preserve all untouched columns;
- reject a missing source field;
- reject a conflicting destination field;
- never silently ignore a missing source;
- never overwrite an unrelated destination;
- preserve row counts and column value lengths.

Runtime validation is defense in depth. Valid production plans should already
have passed schema analysis, but stale or manually constructed plans must still
fail safely.

## Ownership and file boundaries

Expected files:

- `src-tauri/src/node_system/catalog/dataframe/families.rs`
  - register the new non-legacy built-in specification;
- `src-tauri/src/node_system/catalog/dataframe/mod.rs`
  - protocol, parameter contracts, localization, and Rename lowerer;
- `src-tauri/src/node_system/catalog/dataframe/tests.rs`
  - freeze protocol and exact lowered fragment;
- `src-tauri/src/node_system/protocol/types.rs`
  - extend schema expression support only if the current Rename expression
    cannot reference scalar parameters cleanly;
- `src-tauri/src/node_system/compiler/schema_analysis.rs`
  - evaluate and validate parameter-driven rename semantics;
- `src-tauri/src/node_system/compiler/relational.rs`
  - verify real-node fragment merging when needed;
- `src-tauri/src/node_system/runtime/production_relational.rs`
  - strict backend Rename semantics and focused tests;
- `src-tauri/src/project/production_tests.rs`
  - authoritative Source → Rename → Limit production coverage.

Do not add a frontend Rename implementation, compatibility adapter, second
relational planner, or legacy node-definition entry.

## Error handling

Expected blocking cases include:

- blank or whitespace-padded `from`/`to`;
- missing source column;
- destination collision;
- invalid parameter type;
- malformed runtime Rename plan.

Errors use existing structured compiler/runtime error types. Tauri commands
remain unchanged and thin.

## Test strategy

### Catalog and protocol

- exact stable node, port, parameter, category, and localization identities;
- streaming input/output contracts;
- required `from` and `to` parameters;
- static Catalog excludes the node while required parameters lack defaults;
- exact `Input + Rename` lowered fragment.

### Schema analysis

- valid rename preserves unrelated fields and types;
- missing source produces a blocking diagnostic;
- destination conflict produces a blocking diagnostic;
- blank and whitespace-padded names are rejected;
- same-name policy is frozen explicitly.

### Runtime backend

- values and row counts are preserved;
- old name disappears and new name appears;
- missing source and destination conflict fail instead of silently mutating;
- no unrelated field is changed.

### Production integration

A graph assembled only from built-in Registry nodes and an authoritative
GraphDocument executes through `ProjectState::execute_graph` and proves:

- Source → Rename → Limit forms one relational subplan;
- the production relational backend is invoked exactly once;
- no materialization bridge exists;
- because the Rename node output is an independently exposed unbounded root, the shared-source backend suppresses Limit row pushdown for this multi-root graph so Rename remains complete;
- the Rename result contains all renamed rows, while the Limit result contains the expected limited rows;
- reversed document insertion order produces the same plan/result.

### Verification

Run focused catalog, schema/compiler, relational backend, and ProjectState tests
serially, followed by:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Do not rerun the complete Rust suite in this slice. Its previously recorded
failures must be repaired through focused owner-specific work before another
complete attempt is authorized.

## Completion criteria

The slice is complete when:

- `yssbi.dataframe.rename` has a frozen, localized Rust protocol;
- schema and runtime layers enforce identical rename semantics;
- the node lowers to existing relational Rename IR;
- Source → Rename → Limit executes as one production island with one backend
  invocation and zero bridges;
- multi-root planning preserves the complete Rename result and suppresses unsafe
  Limit source pushdown rather than truncating the exposed intermediate root;
- insertion order does not affect the compiled plan or result;
- all focused tests and required gates pass;
- the `TODO.md` node architecture progress table is updated with verified
  evidence.
