# Plugin runtime

> Status: Current
> Scope: 宿主插件安装、授权、进程、任务与结果交接的当前实现
> Canonical owners: 本 crate 的 Plugin Manager 与适配器；体系目标另由[插件 README](../../../plugins/README.md)维护
> Update when: 安装、任务、权限、清理或恢复契约改变时

The host owns installation, grants, process supervision, task admission and result handoff. It
does not link Julia/Bayes implementations. The existing `yss-plugin-protocol` and `yss-plugin-sdk` crates remain under `src-tauri/crates`; plugin
implementations, runtime scripts and system-test fixtures live under `plugins/julia`.

## Module responsibilities

The crate root owns manager construction, the shared registry/runtime state, mutation
admission and durable registry publication. Public `PluginManager`, `PluginLease`
and file-access entry points remain available from the crate root.

| Module                                                                                 | Responsibility                                                                                   |
| -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| [installation](src/installation.rs)                                                    | Package inspection, signing identity, release checks, installation, enable/disable and uninstall |
| [activation](src/activation.rs)                                                        | Shared startup flight, process acquisition, leases and context revocation                        |
| [bridge](src/bridge.rs)                                                                | View contexts, request authorization and host/plugin calls                                       |
| [storage](src/storage.rs)                                                              | Private budgets, cleanup, bounded reads and redirect-safe file resolution                        |
| [package](src/package.rs) and [ledger](src/ledger.rs)                                  | Signed content verification and durable registry/task receipts                                   |
| [process](src/process.rs), [tasks](src/tasks.rs) and [diagnostics](src/diagnostics.rs) | Process supervision, task lifecycle and bounded diagnostic delivery                              |

These modules operate on the same manager-owned state. Process startup and file I/O
retain the existing lock boundaries; file resolution and storage accounting share
the same platform redirect predicate. No second registry or backend is introduced.

## Durable task and installation state

`extensions/registry.sqlite` commits the active registry, terminal task archive and installation
receipts in one SQLite transaction. Startup reads only the current SQLite checkpoint and
creates an empty registry when none exists. Active task admission counts running work only:
the host limit and the granted per-plugin limit never include completed history.

Terminal records are removed from active memory and are queried with a stable history cursor.
History and receipts expire after 30 days. Clearing the history view removes cached result data
while retaining operation identity, parameter hash, terminal state and project receipt until
expiry. Known retries return the original receipt; a forgotten, expired operation ID cannot start
new work. Protocol 2 operation IDs contain their creation time and a nonce.

## Grants, cleanup and trust

The host validates requested budgets and records an effective grant with the installation and
each call context. IPC peers receive that grant during initialization. Snapshot and result byte
limits use this context at the actual data boundary. Private storage is measured at admission,
after host snapshot writes and during active tasks. This is an application soft limit for
`trustedNative`, not an OS sandbox or protection against arbitrary native filesystem access.

View attachment rechecks the captured process instance under the runtime-state lock before
publishing its context. Export grants recheck that the context still exists and its process
instance is active under the same lock used by detach and process-fault revocation. A view
removed during the initial checks cannot gain a new grant, and a process lost during context
preparation cannot publish a new view. Context lookup and publication reuse the same state-owned
process check. Asset reads and host project queries remain outside the runtime-state lock.

Host data reads recheck their original context after the host service returns. If it was revoked
while the read was running, the runtime rejects the reply and releases that context again to
reclaim any resources registered after revocation. Other contexts remain owned by their existing
views or tasks. This check does not rewrite committed result receipts or release acknowledgements.

Cache cleanup uses mutation admission, which rejects startup, other mutations, active process
leases and pending host-to-plugin requests. It then stops the process and revokes idle view contexts before
removing manifest-declared relative directories. It preserves project results, settings outside
those directories and signing identity.
Package collection retains installed digests and digests referenced by unexpired task/install
receipts; unreferenced content-addressed packages can be removed through the management UI.
Path checks reject redirects before host cleanup and do not traverse links while measuring usage.

First installation binds a signing identity. A changed key requires an explicit confirmation
bound to the previous fingerprint and inspected new digest, including after uninstall. Builds
with equal SemVer precedence cannot replace a release with different content. Release versions
come from the source manifest, separately from the complete signed package digest.

## Cancellation and diagnostics

A task cancellation first goes to its plugin task. If cancellation fails, exceeds its grace
period or the process is lost, the host treats it as a process-level fault: all nonterminal tasks
of that exact plugin instance become `outcomeUnknown`, contexts are revoked, and the process tree
is stopped. Late results cannot change those terminal records. Other instances are unaffected.

Stdout remains protocol-only. Stderr is drained into a 64 KiB ring per process, tagged with plugin,
instance and the task identities active at emission. The host retains four recent instances per
plugin and at most 64 buffers globally. Diagnostics are local, capacity-limited and queried
separately from task results; they are not sent to external logging services.
