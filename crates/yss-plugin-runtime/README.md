# Plugin runtime

> Status: Current
> Scope: 宿主插件安装、授权、进程、任务与结果交接的当前实现
> Canonical owners: 本 crate 的 Plugin Manager 与适配器
> Update when: 安装、任务、权限、清理或恢复契约改变时

The host owns installation, grants, process supervision, task admission and result handoff. It
does not link Julia/Bayes implementations. The shared `yss-plugin-protocol` and `yss-plugin-sdk` crates live in the root workspace.
External plugin implementations are installed separately; this repository no longer registers
a test target from a deleted Julia plugin directory.

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
retain the existing lock boundaries; file resolution and storage accounting reuse
`yss-filesystem::metadata_is_redirect` for platform redirect checks. Plugin-relative
path rules and failures remain owned by this crate.

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

The task's `CallContext` owns its parameter hash. Admission compares that same
field for active and archived retries, and Application uses it for result provenance;
the durable task record does not store a second hash.

## Grants, cleanup and trust

The host validates requested budgets and records an effective grant with the installation and
each call context. IPC peers receive that grant during initialization. Snapshot and result byte
limits use this context at the actual data boundary. Private storage is measured at admission,
after host snapshot writes and during active tasks. This is an application soft limit for
`trustedNative`, not an OS sandbox or protection against arbitrary native filesystem access.

View-state writes reuse one serialized buffer for the 64 KiB state limit, private
storage admission and atomic file publication. State path preparation reuses the
storage owner's redirect checks before and after directory creation. Existing
directory/file redirects are rejected before state access.

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

Signed package inspection uses `hex::FromHex` to decode the public key and signature directly
into fixed 32-byte and 64-byte arrays. Both hexadecimal letter cases are accepted; malformed
encoding or length maps to `plugin_package_invalid`, while failed signature verification maps
to `plugin_signature_invalid`.

`current_target()` returns the exact supported build target or `None`: Windows MSVC, Linux GNU/musl,
and macOS on x86_64/aarch64. Inspection rejects packages for any other target. This selector does
not establish platform execution acceptance. On Unix, extraction grants mode `0700` only to the
signed manifest executable after all signed files and the staged manifest have been verified;
archive mode bits do not grant execute permission to other assets.

The [native desktop](../yss-desktop-gpui/README.md) consumes this manager directly for installation,
enable/disable, uninstall, storage maintenance, task history and diagnostics. Manager-owned facts
and receipts remain authoritative. Custom view attachment verifies the signed `.view.json` asset,
reads at most 256 KiB and validates its typed `NativeView` against the manifest before publishing
a session. Project-scoped views require a current project. Command replies validate the same
native contract and recheck their original context after the plugin returns. Existing view-state,
task admission, cancellation and operation receipts remain the backend owners used by native controls.
External plugin implementations and target-platform execution acceptance remain open.

Build the [SDK native example](../yss-plugin-sdk/examples/native_form.rs) and package it from the
repository root for isolated desktop acceptance:

```sh
cargo build --locked -p yss-plugin-sdk --example native_form
cargo run --locked -p yss-plugin-runtime --example native_view_package -- target/debug/examples/native_form /tmp/native-example.yssplugin
```

The [package example](examples/native_view_package.rs) uses a public deterministic example signing
identity, not a release credential. Install through the native manager with an isolated absolute
`YSSBI_APP_DATA_DIR`, then open the declared views, edit controls, calculate, save/reopen, start/cancel
tasks and close/reopen panels. This example does not establish acceptance of external business plugins.

## Cancellation and diagnostics

View and task-start calls rejected before outbound queue admission preserve the process and unrelated
contexts and tasks. The [SDK](../yss-plugin-sdk/README.md) owns that admission boundary and reports
a disconnected writer as `plugin_process_exited`; the process supervisor observes peer closure
and stops the transport. Task cancellation retains the stronger fault policy below because
the host must confirm that remote work has stopped.

After `tasks.start` succeeds, the monitor keeps ownership of the remote work until it validates
a terminal task reply. Temporary `tasks.get` resource exhaustion retries at the existing polling
interval and remains bounded by the task deadline and cancellation grace period. Other observation
failures retire that exact process instance and record its nonterminal tasks as `outcomeUnknown`;
the host cannot claim that unobserved remote work failed. A result or host-handoff rejection after
confirmed remote completion fails only that task, while transport faults retain their process-level
policy. Budget, cancellation and observation failures use the same instance-failure owner once.
Backend task lifecycle regressions live in [tasks/tests.rs](src/tasks/tests.rs).

A task cancellation first goes to its plugin task. If cancellation fails, exceeds its grace
period or the process is lost, the host treats it as a process-level fault: all nonterminal tasks
of that exact plugin instance become `outcomeUnknown` and contexts are revoked. Windows stops the
process tree through its job object. Unix stops the plugin's dedicated process group, including
descendants that retain that group. Its exit observer leaves the leader unreaped until group
termination, keeping the group ID owned; shutdown takes the one Child handle before signaling and
waiting, so repeated stop calls cannot reuse a retired process ID. Waiting and termination occur
outside the handle lock. Target-platform execution acceptance remains open. Late results cannot
change those terminal records. Other instances are unaffected.

Instance faults stop the process and revoke its runtime bindings and export grants before waiting
for task-ledger publication. If the runtime-state mutex is poisoned, teardown uses its retained
contents only for revocation and still reports `plugin_state_unavailable` after cleanup; normal
state access remains unavailable. Publication errors are returned after process retirement and
context release, so a failed or blocked ledger write cannot keep the faulted instance active.
The captured process identity continues to protect replacement instances and completed receipts.

Stdout remains protocol-only. Stderr is drained into a 64 KiB ring per process, tagged with plugin,
instance and the task identities active at emission. The host retains four recent instances per
plugin and at most 64 buffers globally. Diagnostics are local, capacity-limited and queried
separately from task results; they are not sent to external logging services.

Focused installation validation uses:

```sh
cargo test -p yss-plugin-runtime --test installation
cargo test -p yss-plugin-runtime --lib process::unix_process_tests::
cargo clippy -p yss-plugin-runtime --lib --test installation --no-deps -- -D warnings
cargo fmt -p yss-plugin-runtime -- --check
```
