# Project registry workflows

> Status: Current
> Scope: Project registration, path admission and listing
> Canonical owners: This crate owns registry workflows; the registry contract owns records and storage ports
> Update when: Registration, path checks or listing behavior changes

`register_project` admits an existing metadata file and binds its native directory identity.
Metadata redirects are rejected through the shared Filesystem predicate. Reopening a registered
root retains its registration ID and name; root replacement is rejected. Canonical paths
preserve native Windows prefixes needed to reopen long or special paths. Registration
revalidates the captured root after the asynchronous lookup before comparing its identity
with the persisted record.

Path normalization returns `ProjectRegistryError::InvalidPath`; default parent lookup returns
the standard environment error. Host adapters own localized messages. Listing sorts numeric
Unix timestamps, then names.

Records and the storage port belong to
[Registry Contract](../yss-project-registry-contract/README.md). Application owns project
activation; Filesystem owns directory identity and generic file access.

```sh
cargo test -p yss-project-registry --lib
cargo clippy -p yss-project-registry --lib --tests --no-deps -- -D warnings
cargo fmt -p yss-project-registry -- --check
```
