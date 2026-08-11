# DataFrame Decompose Schema Interface Design

## Problem

The `yssbi.dataframe.decompose` node declares a derived `columns` output template
using the interface resolver ID
`yssbi.dataframe.interface.columns`. The built-in provider advertises that ID,
but the runtime `InterfaceResolverSet` only installs function-interface
resolvers. Compilation therefore emits
`compiler.interface.resolver_missing` as soon as the node is analyzed.

Registering an empty resolver would remove the immediate diagnostic but would
not implement the node correctly. The output ports must correspond to the
resolved schema at the node's `dataframe` input, including schemas transformed
by upstream project, rename, filter, and other schema-preserving DataFrame
nodes.

The current compiler resolves dynamic interfaces while initially visiting each
node, before graph-wide schema analysis. A DataFrame columns resolver therefore
cannot consume the authoritative input schema without changing compilation
staging.

## Goals

- Register a concrete implementation for
  `yssbi.dataframe.interface.columns`.
- Materialize one `DataSeries` output candidate per field in the authoritative
  schema of the `dataframe` input.
- Support direct database sources and schemas transformed by upstream
  DataFrame nodes.
- Keep `SchemaAnalyzer` as the only implementation of schema propagation and
  transformation semantics.
- Preserve stable dynamic bindings across recompilation when field identity is
  stable.
- Produce schema or resource diagnostics when columns cannot be resolved,
  rather than incorrectly reporting a missing resolver.

## Non-goals

- Reimplementing schema propagation inside the interface resolver.
- Changing the runtime representation of DataFrames or DataSeries.
- Changing the behavior of user-created dynamic ports.
- Refactoring unrelated compiler phases or legacy DataFrame kernels.
- Adding compatibility behavior for obsolete node protocols.

## Chosen approach

Use staged interface and schema analysis.

The compiler will first establish a provisional graph interface sufficient to
analyze upstream DataFrame schemas. Resolvers that do not depend on schema,
such as function-interface resolvers, may resolve during this stage. A
preliminary schema pass then computes schema facts for DataFrame inputs. The
compiler subsequently invokes schema-dependent interface resolvers with access
to those facts, materializes derived ports, and runs the final validation,
type-analysis, and schema-analysis stages against the complete interface.

This avoids recursive graph traversal and duplicated schema semantics inside
the DataFrame resolver.

## Compiler staging

Compilation analysis will use the following logical stages:

1. Resolve node registrations and normalize parameters.
2. Build provisional interfaces:
   - declared ports are available;
   - user-created instances are available;
   - existing bindings for schema-dependent derived templates are retained
     provisionally;
   - resource-only dynamic interfaces continue to resolve normally.
3. Validate the provisional data connections required for schema propagation.
4. Run preliminary schema analysis using the existing `SchemaAnalyzer` and
   registered `SchemaResolverSet`.
5. Resolve schema-dependent dynamic interfaces using the preliminary resolved
   schema facts.
6. Build the final resolved interfaces and dynamic interface projections.
7. Run authoritative connection validation, input-binding validation, cycle
   validation, type analysis, and final schema analysis.
8. Lower only when the final analysis has no blocking diagnostics.

The preliminary pass is an internal dependency-resolution stage. Its facts are
not published as final analysis results. Final published facts and diagnostics
come from the authoritative pass after dynamic ports have materialized.

## Resolver API

Extend dynamic interface resolution context so a resolver can access the
resolved schema associated with a declared input port. The API should expose
read-only schema facts, not the mutable compiler analysis state.

A resolver that requires an unavailable schema must return a classified
resolution result rather than pretending its implementation is absent:

- a resource failure retains the exact resource key and reason;
- an unresolved or invalid upstream schema produces an interface/schema
  resolution diagnostic associated with the node or input port;
- `InterfaceResolverMissing` remains reserved for an ID that has no registered
  implementation.

The exact API shape may use a schema-fact lookup callback or a borrowed map of
`PortAddress` to `ResolvedSchemaFact`. It must not give resolvers access to UI
state, project locks, or mutable graph documents.

## DataFrame columns resolver

Add a `DataframeColumnsResolver` registered under
`DATAFRAME_COLUMNS_RESOLVER`.

For `yssbi.dataframe.decompose`, it will:

1. Locate the declared `dataframe` input address for the current node.
2. Read its preliminary `ResolvedSchemaFact`.
3. Preserve the field order from that fact.
4. Return one `InterfaceResolverMember` per field.
5. Use the field name as the visible label.
6. Produce a `DynamicMemberLocator::SchemaField` for binding identity.
7. Mark identity as stable when the schema field has a stable source identity;
   otherwise use `SchemaFieldIdentityGuarantee::SnapshotScoped`.

Schema source identity must be derived from compiler-owned schema lineage, not
from a display label alone. Two different fields with the same label must not
be treated as the same persistent member. If current schema facts do not carry
sufficient lineage, the implementation should add the minimum compiler-owned
lineage metadata required to distinguish source and field identities.

## Registration

The provider declaration and runtime implementation must remain consistent:

- `build_provider_fragment()` continues advertising
  `DATAFRAME_COLUMNS_RESOLVER` because the protocol references it.
- The built-in resolver builder installs `DataframeColumnsResolver` under the
  same validated `InterfaceResolverId`.
- Resolver registration tests assert that every built-in resolver ID referenced
  by a built-in protocol has an implementation in production compilation.

The builder may be renamed from its function-specific name if necessary to
reflect that it installs all built-in interface resolvers. Call sites in
project compilation and function-plan publication must use the complete set.

## Dynamic binding lifecycle

When a schema remains unchanged, previously materialized column ports resolve
through exact locator identity.

When a field disappears:

- an existing bound port becomes an orphan and retains its last-known label;
- its persistent connection is not silently rebound to another field with the
  same display name.

When a new field appears:

- it is exposed as an unbound materialization candidate;
- no persistent document mutation occurs until the existing authorized
  materialization workflow accepts it.

When a renamed field has different schema identity, it is treated as removal
plus addition unless schema lineage explicitly guarantees identity
preservation across rename.

## Error handling

- Missing resolver implementation:
  `compiler.interface.resolver_missing`.
- Database or other resource unavailable: preserve
  `compiler.resource.resolution_failed` with the exact resource key.
- Upstream schema unavailable or invalid: emit a blocking schema/interface
  diagnostic at the `dataframe` input or owning node.
- Resolver implementation failure unrelated to a resource:
  `compiler.interface.resolver_failed`.
- Duplicate generated locators continue using existing dynamic-interface
  duplicate diagnostics.

The compiler must not fabricate zero columns as a successful result when the
schema could not be resolved, because that would hide an invalid graph and may
incorrectly orphan persisted ports.

## Tests

Add focused Rust regression coverage for:

1. The built-in DataFrame columns resolver is installed.
2. A database source connected directly to Decompose exposes all database
   columns in schema order without `resolver_missing`.
3. Rename followed by Decompose exposes the renamed field.
4. Project followed by Decompose exposes only projected fields in project
   order.
5. Schema-preserving filtering followed by Decompose retains all fields.
6. Removing a source field orphans its existing dynamic output.
7. Adding a source field exposes an unbound materialization candidate.
8. A missing database produces a resource diagnostic, not
   `compiler.interface.resolver_missing`.
9. A genuinely unregistered resolver still produces
   `compiler.interface.resolver_missing`.
10. Production project compilation and function-plan publication both use the
    complete resolver set.

Where possible, tests should exercise `GraphCompiler::with_resolvers` using the
same resolver construction path as production rather than testing the resolver
in isolation only.

## Validation

Run from the repository root:

```text
pnpm rust:test <focused test filters for DataFrame interface resolution>
pnpm rust:check
git diff --check
```

Because the change is Rust-only, `pnpm verify` is not required unless the
implementation also changes frontend contracts.

## Risks and mitigations

### Preliminary and final schema passes diverge

The final pass remains authoritative. Tests must verify that dynamic outputs
match final resolved schemas for representative transforms. Both passes must
use the same `SchemaAnalyzer` and resolver set.

### Existing dynamic resolvers regress

Function-interface resolution does not depend on schema and must retain its
current behavior. Existing function resolver tests remain part of focused
validation.

### Resource reads are counted twice

Tracked resource resolution must remain deterministic. Compilation basis and
observations should represent the set of resources read, not duplicate entries.
Tests should verify stable compilation basis across the staged passes.

### Invalid provisional connections pollute final diagnostics

Preliminary validation should gather only what is required to compute schema.
Only final authoritative diagnostics are published, except resource failures
that prevent schema-dependent interface resolution and are still present in
final analysis.

## Acceptance criteria

- Connecting a valid DataFrame to `拆分数据框` never reports a missing interface
  resolver.
- The node offers output ports matching the final upstream DataFrame schema,
  including after project, rename, and filter transformations.
- Dynamic binding and orphan behavior is identity-safe.
- Missing resources and invalid schemas produce accurate blocking diagnostics.
- Focused regression tests, `pnpm rust:check`, and `git diff --check` pass.
