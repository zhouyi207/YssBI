# Project resource contracts

> Status: Current
> Scope: Resource identities, mutation requests, function documents and publication deltas
> Canonical owners: This crate owns shared resource values; Project owns state and publication
> Update when: Resource identities, function signatures or publication contracts change

`ResourceKey` identifies the target of a typed `MutationRequest`. Its base resource revision and
operation ID remain internal consistency inputs for the existing Project mutation gate;
Application and native adapters capture and validate them at their boundaries.

Function documents and signature patches carry parameter identities, declared types and the
signature revision. Resource deltas describe committed function, chart, database, lifecycle or
path changes. Producers and consumers share these values without maintaining a second resource
classification or deriving undo commands from publication events.

[Project](../yss-project/README.md) owns current resource state, graph editing history, versions
and publication. Database editing history belongs to
[Database Runtime](../yss-database-runtime/README.md). This crate supplies values and mutation
failures; transaction rollback and recovery stay with their existing owners.

Run focused checks from the repository root:

```sh
cargo test -p yss-project-history --lib
cargo clippy -p yss-project-history --lib --tests --no-deps -- -D warnings
cargo fmt -p yss-project-history -- --check
```
