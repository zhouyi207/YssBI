# Relational contract

> Status: Current
> Scope: Neutral relation and series handles, operation requests, execution control and grouping sessions
> Canonical owners: This crate owns shared contracts; concrete plans and queries belong to the engine
> Update when: Handle, operation, control or grouping contracts change

`RelationHandle` and `SeriesHandle` share immutable plans and their row-domain identity.
Callers request projections, transforms, aggregates and grouping through these handles.
`RelationFactory` imports materialized Arrow values and freezes explicit result boundaries;
`RelationExecutor` supplies controlled reads. This crate has no Graph, Project, UI or DataFusion dependency.

`SeriesTransform` describes an operation request. Standardization accepts an evaluated mean and sample
standard deviation, as does inverse standardization. The caller obtains statistics through
`SeriesReduction::StandardizationStatistics` and consumes that relation once. The engine validates finite
statistics and a positive deviation, preserves null positions and builds a projection using those exact
statistics. The transform does not calculate them again.

`RelationControl` carries shared cancellation, a deadline and the caller's memory policy.
Read adapters check it while preparing and consuming batches. Optional trait operations return typed
unsupported-input errors until an adapter implements them.

Deferred schemas must be resolved before interpreting their fields; an unresolved empty schema is distinct
from a resolved table with zero columns. Group sessions retain their source and keys, stream groups and
combine function outputs under the same control. Resource authorization and graph result ownership stay
with their domain owners.

[Kernel adapters](../yss-node-kernel/README.md) · [Concrete engine](../yss-database-engine/README.md)
