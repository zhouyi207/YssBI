# Function editor projection

> Status: Current
> Scope: Read projection of Project's persisted function signatures
> Canonical owners: [src/lib.rs](src/lib.rs) owns pin and projection shapes and their pure mapping
> Update when: Projection fields, mapping or consumers change

`FunctionEditorProjection::try_from` maps a `FunctionDocument` into its typed
resource revision and ordered input/output pins. Project index reads and
Application graph receipts use this projection. It holds no editable signature,
project state or delivery subscription.

Type names are parsed by [Data Contract](../yss-data-contract/README.md)'s
`ValueType` implementation, and failures retain `ValueTypeParseError`. This
crate does not own another type grammar or validation pass. Application's graph
catalog also consumes the Data Contract parser directly when constructing its
semantic function contracts.

Pin IDs, names, order and the resource revision come from the document. A
declared return type produces the `return` output pin. Project owns signature
updates and publication; the native host owns presentation and interaction.

Run checks from the repository root:

```sh
cargo test -p yss-function-editor-projection --lib
cargo test -p yss-application --lib graph::catalog::tests::
cargo check -p yss-desktop-gpui --bin yss-desktop-gpui
cargo clippy -p yss-function-editor-projection --lib --tests --no-deps -- -D warnings
```
