# Node protocol

> Status: Current
> Scope: Node declarations, semantic identities, port and parameter contracts, type expressions and literal validation
> Canonical owners: This crate owns graph-independent node declarations and validation; Registry owns registration and Graph owners hold resolved facts
> Update when: Node declaration fields, configuration projections or value validation change

`NodeProtocol` declares catalog metadata, interfaces, parameters, typing and execution semantics.
Semantic IDs validate their local or namespaced spelling at construction and deserialization.
Port cardinalities and member groups share the same bounds for initial creation and later
pin operations. Parameters own defaults, conditions, constraints and partial value merging.
Configuration JSON Schema projects these declarations without loading a graph or executing a node.

`TypeExpr` represents declared patterns; normalization flattens, deduplicates and orders unions.
`TypedValue` carries a declared value type and the shared Data Contract value. Literal validation
checks type acceptance and value shape through the caller's nominal validation context.
JSON arrays infer one homogeneous element type without retaining or sorting every element's
type. An empty array uses a fully resolved declared element type; unresolved generic or class
patterns still require values from which to infer a type.

Dataframe nominal codecs own exact column selections and tagged filter predicates. They reuse
Data Contract column-name and scalar literal validation. Registered type classes, providers,
nominal codecs and resolver implementations belong to Registry; connected schemas, lineage,
diagnostics and resolved graph types belong to Graph Analysis.
Built-in definitions and localization are documented by
[Node Catalog](../yss-node-catalog/README.md). Protocol has no Graph, Project or GUI dependency.

```sh
cargo test -p yss-node-protocol --lib
cargo clippy -p yss-node-protocol --lib --tests --no-deps -- -D warnings
cargo fmt -p yss-node-protocol -- --check
```
