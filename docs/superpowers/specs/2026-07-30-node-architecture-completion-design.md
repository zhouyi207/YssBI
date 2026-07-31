# Node Architecture Completion Design

## Status and scope

This design completes the load-bearing production paths still missing from
`docs/plan/node-architecture.md`. Phases 1–4 are treated as established
foundations. Existing Phase 5–9 code is reused rather than replaced.

This delivery includes:

1. a production relational vertical slice;
2. graph-scoped compile publication;
3. production structured-control coverage;
4. a localized static Catalog frontend;
5. resource-aware Catalog creation and documentation;
6. bounded production compile/run observability.

The following optimizations are explicitly deferred:

- parallel operation scheduling;
- cross-run caches;
- the complete cache identity/fingerprint system;
- run-ID cancellation commands;
- timeout or forced termination for synchronous kernels;
- external telemetry backends;
- federated relational backends;
- cost-based relational optimization.

## Global architecture constraints

- Rust remains authoritative for Node Protocol, Registry, GraphDocument,
  semantic analysis, execution plans, project resources, mutation validation,
  and stable identity allocation.
- React consumes localized Catalog DTOs, editor projections, diagnostics, trace
  projections, and committed mutation results. It never reconstructs protocol
  semantics.
- `ProjectState.project_data` remains authoritative project state.
- `ProjectState::insert_graph` remains the only graph insertion path.
- `ProjectPublicationCoordinator` remains the sole frontend project identity,
  activation watermark, and resource publication owner.
- Frontend services remain the only IPC owners.
- No compatibility command, fallback reader, dual executor, dual publication
  owner, frontend schema registry, or frontend dynamic-port inference may be
  introduced.
- The following remain prohibited: `get_editor_schema_command`, legacy schema
  stores, frontend node registry stores, frontend global type-system stores,
  and title/localization-derived node identity.

## Delivery structure

The implementation is divided into six sequential vertical slices:

1. Relational execution
2. Compile publication
3. Structured control production closure
4. Static Catalog frontend
5. Resource Catalog and documentation
6. Production observability

Every slice has a focused TDD and review gate. A slice is complete only when a
real production entry point is covered; isolated IR or planner tests are not
sufficient. Each slice receives its own implementation plan so the relational,
compile-publication, control, Catalog, and observability write sets stay bounded.
The plans are executed sequentially in the order above because later slices rely
on production contracts established by earlier slices.

## Slice 1: relational execution vertical slice

### Supported graph

The first production slice supports this pipeline:

```text
DataFrame Source -> Limit
```

Project and Filter operators may be added in later slices, but are not required
for this first completion boundary.

### Lowering

The built-in Source and Limit node implementations produce
`LoweredKernel::Relational` fragments targeting `relational.default`.
`RelationalPlanner` must combine a compatible Source and Limit into one island.
The compiler continues to own fragment ordering, bridge derivation, and
subplan identity.

### Backend contract

The production backend owns a lazy relational expression. It must not convert
Source into a complete protocol object before applying Limit. Limit pushdown
must change source scanning behavior rather than remain an unused hint.

Backend execution returns:

- root outputs;
- fragment outputs needed by downstream islands;
- exact bridge values when a consumer contract prevents island merging.

Cancellation is checked during source scan, operator evaluation, bridge
materialization, and final result materialization.

### Acceptance

Tests using the real built-in Catalog, GraphDocument, ProjectState, compiler,
and `ProjectState::execute_graph` prove:

- a Source -> Limit graph generates one relational island and invokes the backend
  exactly once;
- Source and Limit have no internal materialization bridge;
- pushdown changes the source scan limit;
- a separate graph whose consumption contract forces two islands publishes the
  producer fragment output and consumes it as the exact downstream bridge input;
- cancellation at each backend checkpoint prevents publication of a completed
  result.

## Slice 2: graph-scoped compile publication

### Ownership

Every loaded `graphPath` owns one production compile slot through
`CompileCoordinator`. The existing two-product coordinator shape is retained:
its analysis payload becomes a focused `PublishedCompileAnalysis` containing the
current `AnalysisSnapshot` and optional `ValidatedSemanticGraph`, while its plan
payload remains the optional `ExecutionPlan`. Both projections carry one exact
`CompilationBasis` and compile ID. A blocking analysis publishes
`PublishedCompileAnalysis { semantic: None, ... }` and clears the plan.

The compile product key is `graphPath`, never a tab ID, title, or localized
string.

### Publication protocol

`CompilationBasis` includes graph revision, Registry fingerprint, and resource
versions. A product publishes only when its complete basis equals the current
basis.

When a newer request arrives, the coordinator cancels or coalesces older work
and retains the latest request. A blocking diagnostic publishes analysis and
clears any previously executable plan. It must be impossible for a stale
compile to restore an older plan.

Editor projection and execution consume the same current compile product. They
trigger compilation only when no reusable current product exists.

Graph unload, project replacement, Registry replacement, and relevant resource
revision changes cancel or invalidate affected slots.

### Determinism

Canonical differential tests construct semantically identical documents using
different node, connection, and map insertion orders. They require identical:

- AnalysisSnapshot serialization;
- ValidatedSemanticGraph serialization;
- ExecutionPlan serialization;
- diagnostic ordering;
- relational fragment and subplan ordering.

## Slice 3: structured control production closure

The existing control IR and scheduler remain authoritative. This slice adds
real-Catalog production coverage rather than a second control implementation.

Required end-to-end graphs cover:

- Branch: only the selected branch runs, including effect operations, and
  branch results bind correctly;
- Loop: initial/next/result carried values bind correctly, cancellation is
  checked at each iteration, and the iteration limit returns a structured
  error;
- Call: the current function plan generation is used, arguments/results bind
  correctly, frames are independent, and recursion limits apply;
- Effect ordering: explicit effect dependencies determine execution order,
  insertion order does not, failures are not retried, and acquired resources
  are released.

This slice does not implement operation parallelism, an effect-aware thread
pool, forced kernel termination, or a retry-policy DSL.

## Slice 4: static Catalog frontend

### IPC and store

`CatalogService.getLocalizedCatalog(projectInstanceId, locale)` invokes the
existing Rust Catalog command. The command validates the supplied project
instance before taking its snapshot. Slice 4 extends the Catalog response
envelope with `projectInstanceId`, `registryFingerprint`, and
`resourcePublicationRevision`; these fields exist before resource-bound items are
enabled so cache identity is never inferred by React. The frontend defines
generated or contract-tested DTOs for localized categories, items, and creation
descriptors.

The minimum frontend structure is:

```text
src/services/nodeSystem/catalogService.ts
src/features/domain/nodeCatalog/identity.ts
src/features/domain/nodeCatalog/catalogItem.ts
src/features/domain/nodeCatalog/creationDescriptor.ts
src/features/domain/nodeCatalog/search.ts
src/features/core/nodeCatalog/nodeCatalogStore.ts
src/features/core/nodeCatalog/localizedSearchIndex.ts
src/features/core/nodeCatalog/selectors.ts
src/features/application/nodeCatalog/createNodeFromDescriptor.ts
src/features/application/nodeCatalog/useLocalizedNodeCatalog.ts
```

The store caches by project identity, locale, Registry fingerprint, and resource
publication revision.

### Static creation descriptor

```ts
type NodeCreationDescriptor = {
  kind: 'static';
  nodeTypeId: string;
};
```

The adapter sends only node type ID, canvas position, empty parameters, current
graph revision, and operation ID. Rust resolves protocol defaults and allocates
node, port, and dynamic-instance identities. The frontend never sends ports,
pin IDs, inferred types, dynamic interfaces, or arbitrary parameter maps.

### Search

Search uses the current localized item fields supplied by Rust:

- title;
- aliases;
- technical terms;
- normalized search text;
- optional pinyin.

It does not index other locales, description text, documentation text, or
frontend-derived type compatibility.

### Capability boundary

After this slice:

```text
createNodes = true
catalogDescriptors = true
resourceBoundDescriptors = false
contextualCompatibility = false
documentation = false
```

## Slice 5: resource Catalog and documentation

### Resource-aware backend projection

The Catalog command snapshots authoritative ProjectState resources and invokes
`BuiltinCatalog::localize_with_resources`. The response envelope introduced in
Slice 4 remains unchanged; Slice 5 populates resource-bound items from the same
snapshot revision. The first entries are user functions, global variables, and
database-backed DataFrame sources. A database entry binds the existing DataFrame
source node type to that database resource; it does not introduce a second
frontend DataFrame identity model.

Resource-bound descriptors preserve the existing Rust descriptor vocabulary and
add the revision required for mutation-time validation:

```ts
type ResourceBoundCreationDescriptor = {
  kind: 'resourceBound';
  nodeTypeId: string;
  resourcePath: string;
  resourceRevision: number;
  createArgs: { kind: 'function' | 'variable' | 'resource' };
};
```

`resourcePath` is the backend-issued stable resource address. React treats it as
opaque and never derives it from a display name or locale.

A Catalog snapshot supports discovery only. Mutation application revalidates
project identity, graph revision, node type, resource identity/revision, scope,
and parameter constraints.

### Documentation

`NodeDocumentationModal` consumes the current localized Catalog item and shows
title, description, documentation, localized port/parameter projections, and
stable technical IDs. It does not request a legacy schema or merge all locale
bundles in React. Documentation bodies remain excluded from default search.

### Legacy removal

After static and resource-bound creation are live, production removes:

- legacy NodeDefinition-based creation;
- `resolveEffectiveDefinition`;
- frontend Call Function pin generation;
- contextual Catalog type inference;
- all-language documentation indexing;
- unavailable palette/create placeholder paths.

Contextual compatibility may return later only as a Rust projection; this
scope does not add a frontend inference replacement.

## Slice 6: bounded production observability

A project-scoped bounded ring buffer implements the existing compile/run trace
sink interfaces. No external telemetry dependency is introduced.

Trace correlation includes project session ID, graph path, complete compilation
basis, compile ID, run ID, source node ID/type, and parent call ID. Required
span families are snapshot, analysis, lowering, resource acquire, operation,
relational backend, and cleanup.

Read-only IPC provides:

```text
list_graph_traces(projectInstanceId, graphPath)
get_run_trace(projectInstanceId, runId)
```

The frontend adds a service and a focused developer details projection. It
cannot mutate, delete, or fabricate backend traces.

## Error model

Stable error codes are used at application and IPC boundaries.

Compile errors:

```text
compile_cancelled
compile_basis_stale
compile_blocked
compile_publication_conflict
```

Relational errors:

```text
relational_lowering_failed
relational_backend_unavailable
relational_bridge_missing
relational_execution_failed
```

Catalog errors:

```text
catalog_project_stale
catalog_resource_stale
catalog_descriptor_invalid
catalog_locale_unavailable
```

Trace errors:

```text
trace_not_found
trace_project_stale
```

Compile-blocked results retain the current analysis diagnostics and never expose
an older plan. Cancellation and stale results do not produce ordinary error
toasts. Relational failures correlate subplan, source node, run, and basis.

## Test strategy

Applicable layers are required for every slice:

1. pure deterministic unit tests;
2. Rust ProjectState application tests;
3. command/service wire contract tests;
4. real production vertical-slice tests.

Focused gates are:

- relational: compiler relational, production backend, ProjectState execute,
  Rust check and format;
- compile publication: coordinator, editor projection, execution basis/CAS, and
  stale compile concurrency;
- control: compiler control, runtime structured region, and real ProjectState
  execution;
- Catalog: Rust localization/command, frontend service/store/search/palette/
  create, and TypeScript typecheck;
- observability: retention, correlation, stale-project rejection, span coverage,
  and focused frontend service/view tests.

After all six slices pass focused review, the complete Rust suite runs exactly
once with `CARGO_BUILD_JOBS=1 pnpm rust:test -- --test-threads=1`. It is not run
between slices. An OOM, stall, or timeout is recorded and is not retried in the
same task. Frontend tests always use explicit test-file lists; unqualified
`pnpm test` and `pnpm verify` are not used for this delivery.

## Completion criteria

This delivery is complete only when:

- built-in relational nodes execute through the production relational backend;
- editor projection and execution reuse graph-scoped compile products;
- Branch, Loop, Call, and effect semantics pass real-Catalog production tests;
- the palette searches and creates static and resource-bound nodes by stable ID;
- the documentation modal consumes localized Catalog items;
- legacy frontend node-definition inference is absent from production;
- production compile/run paths no longer use only `NOOP_TRACE_SINK`;
- Rust source and frontend contract audits block old architecture paths;
- all focused gates pass;
- the one complete-suite result is recorded accurately.
