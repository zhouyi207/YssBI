# Project registry contract

> Status: Current
> Scope: Project registration records and the persistence port
> Canonical owners: This crate owns shared records and storage methods; Registry owns path admission and workflows
> Update when: Record fields, serialization or the persistence contract changes

`ProjectRecord` carries a registration ID, canonical metadata path, presentation fields and
native root identity with its validity state. Serialization uses camelCase and rejects unknown
fields. `deletion_identity` requires a valid, nonempty identity; lifecycle owners perform the
remaining deletion checks.

`ProjectRegistryStore` loads the complete catalog for listing and cleanup, reads individual
records by registration ID or canonical metadata path, and upserts or removes records.
Individual reads return `None` for a missing record and a typed error for storage failure.
Removing a missing record returns `Unavailable`. Storage does not normalize paths or infer
filesystem identity.

The [Registry workflows](../yss-project-registry/README.md) admit paths and revalidate roots.
The [SQLite adapter](../yss-project-registry-sqlite/README.md) implements the port; Application
injects it. No GUI or concrete storage dependency belongs to this contract.

```sh
cargo test -p yss-project-registry-contract --lib
cargo fmt -p yss-project-registry-contract -- --check
```
