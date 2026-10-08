# Project registry workflows

> Status: Current
> Scope: Project discovery, registration, path admission, listing and cleanup
> Canonical owners: This crate owns registry workflows; the registry contract owns records and storage ports
> Update when: Registration, discovery, path checks or cleanup behavior changes

`register_project` admits an existing metadata file and binds its native directory identity.
Metadata redirects are rejected through the shared Filesystem predicate. Reopening a registered
root retains its registration ID, name and favorite state; root replacement is rejected.
Canonical paths preserve native Windows prefixes needed to reopen long or special paths.

Discovery uses the private `discovery` module and shared cancellation/progress values. It skips
redirected metadata and directory trees. Cleanup retains a record only when its root identity
still matches and its metadata entry is a valid existing file. Cleanup removes registration
records and leaves project files with their lifecycle owner.

Path normalization returns `ProjectRegistryError::InvalidPath`; default parent lookup returns
the standard environment error. Host adapters own localized messages. Listing sorts favorites,
numeric Unix timestamps and names; individual lookups do not invoke the presentation sort.

Records and the storage port belong to
[Registry Contract](../yss-project-registry-contract/src/lib.rs). Application owns project
activation and deletion; Filesystem owns directory identity and generic file access.

```sh
cargo test -p yss-project-registry --lib
cargo clippy -p yss-project-registry --lib --tests --no-deps -- -D warnings
cargo fmt -p yss-project-registry -- --check
```
