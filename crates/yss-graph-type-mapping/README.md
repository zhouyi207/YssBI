# Graph type mapping

> Status: Current
> Scope: Conversion between shared data types and Graph type expressions or resolved types
> Canonical owners: This crate owns representation conversion; Data Contract and Node Protocol own the represented types
> Update when: A data type or graph type mapping changes

`type_expr_from_data_type` maps scalar semantic IDs, objects, dataframes, named structs,
arrays, data series, unions and unknown types into Node Protocol expressions. Union members
retain their input order; declaration normalization remains the Protocol owner's operation.
DataSeries uses the Protocol constructor ID. Dynamic struct IDs pass through semantic ID
validation and return a typed error when invalid.

`data_type_from_resolved_type` converts nominal semantic types and unary Array/DataSeries
constructors back to Data Contract values. Other nominal IDs represent named structs;
unsupported applied constructors or arities return `None`. `relational_scalar_type_from_data_type`
only exposes scalar semantics and leaves composite types unknown.

This crate does not register types, infer node types or hold graph state. The underlying
contracts belong to [Data Contract](../yss-data-contract/README.md) and
[Node Protocol](../yss-node-protocol/README.md); graph semantics belong to
[Graph Analysis](../yss-graph-analysis/README.md).

```sh
cargo test -p yss-graph-type-mapping --lib
cargo clippy -p yss-graph-type-mapping --lib --tests --no-deps -- -D warnings
cargo fmt -p yss-graph-type-mapping -- --check
```
