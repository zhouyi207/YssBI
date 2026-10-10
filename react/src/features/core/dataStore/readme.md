# Data Store reference

> Status: Historical
> Scope: Read-only projection preparation in the retained React source
> Canonical owners: These helpers own reference projection preparation, not current Rust domain state
> Update when: Reference ownership or current contract routing changes

This directory is not a native build entry. See the [root README](../../../../../README.md),
[documentation index](../../../../../docs/README.md),
[Graph application](../../../../../crates/yss-application/src/graph/README.md) and
[GPUI host](../../../../../crates/yss-desktop-gpui/README.md) for current contracts.

In the reference implementation, `graphProjection.ts` prepares read-only graph sessions,
entity buckets and result summaries; `graphMeta.ts` prepares graph-kind and function-signature
projections. Neither helper owns a separate writable store.
`../resource/resourceStore.ts` publishes these candidates with resource and document state,
including database declarations and metadata. Unchanged branches retain their references.

The reusable boundary is atomic publication of a validated candidate: consumers must not
observe half-installed graph/resource state or turn a read projection into another document
or history authority. Request admission, project identity and late-response checks belong to
the reference Application layer. These JavaScript storage choices are not requirements for
native Rust state or evidence that any view has been migrated.
