# Project identity

> Status: Current
> Scope: Stable identity values shared by Project, Application and adapters
> Canonical owners: [src/lib.rs](src/lib.rs) owns the public identity surface; individual source modules own each value
> Update when: Identity roles, serialization or revision advancement changes

This crate holds no project state, I/O, operation ledger or UI lifecycle.
`ProjectInstanceId` identifies an activation; `ProjectSessionId` identifies its
replaceable runtime. `ProjectRegistrationId` identifies a persisted registry
row, and `ProjectRootIdentity` preserves an opaque native directory identity.
These roles remain separate even when their serialized representations are
strings.

`OperationId` identifies one operation. `ResourceRevision` supports checked
monotonic advancement and reports exhaustion without wrapping. Project owns
resource publication and index publication order; this crate does not keep a
parallel project revision model.

`ProjectResourceRef` provides resource kind and opaque identity to automation
and workbench intents. `ProjectResourcePath` is also an opaque string value;
resource owners validate their own graph, chart, document or database paths.

Run focused checks from the repository root:

```sh
cargo test -p yss-project-identity --lib
cargo clippy -p yss-project-identity --lib --no-deps -- -D warnings
cargo check -p yss-desktop-gpui --bin yss-desktop-gpui
cargo fmt -p yss-project-identity -- --check
```
