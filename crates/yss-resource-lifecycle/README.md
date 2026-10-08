# Resource lifecycle admission

> Status: Current
> Scope: Project-scoped graph/chart lifecycle owners, token admission and guard rollback
> Canonical owners: This crate owns lifecycle registration; Project owns filesystem publication and session checks
> Update when: Token admission, supersession, guard commit or rollback behavior changes

`ResourceLifecycleRegistry` admits graph and chart operations independently by project and
resource path. External token watermarks reject replay; internal token allocation retains its
own issued watermark and does not advance the client's sequence. Registration identities are
not reused after project state retirement.

The boundary validates current registration and commits only guards belonging to the same
Registry state. Rejecting a foreign guard preserves both registrations and its Drop release.
Dropping an uncommitted guard restores the nearest eligible predecessor and compacts abandoned
registrations. Key comparisons borrow owner fields while inspecting registration chains.

Project performs session validation and file publication around this admission boundary.
Lifecycle registration does not read or write project files or replace the graph document owner.

```sh
cargo test -p yss-resource-lifecycle --lib
cargo clippy -p yss-resource-lifecycle --lib --tests --no-deps -- -D warnings
cargo fmt -p yss-resource-lifecycle -- --check
```
