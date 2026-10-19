# Project registry contract

> Status: Current
> Scope: Project registration records and the persistence port
> Canonical owners: This crate owns shared records and storage methods; Registry owns path admission and workflows
> Update when: Record fields or the persistence contract changes

`ProjectRecord` carries a registration ID, canonical metadata path, presentation fields and
native root identity with its validity state. `valid_root_identity` returns the identity only
when it is valid and nonempty.

`ProjectRegistryStore` loads the complete catalog for listing, reads one record by canonical
metadata path and upserts records. A path read returns `None` for a missing record and a typed
error for storage failure. Storage does not normalize paths or infer filesystem identity.

The [Registry workflows](../yss-project-registry/README.md) admit paths and revalidate roots.
The [SQLite adapter](../yss-project-registry-sqlite/README.md) implements the port; Application
injects it. No GUI or concrete storage dependency belongs to this contract.

```sh
cargo test -p yss-project-registry-contract --lib
cargo fmt -p yss-project-registry-contract -- --check
```
