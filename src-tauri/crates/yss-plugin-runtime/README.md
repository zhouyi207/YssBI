# Plugin runtime

The host owns installation, grants, process supervision, task admission and result handoff. It
does not link Julia/Bayes implementations. The existing `yss-plugin-protocol` and `yss-plugin-sdk` crates remain under `src-tauri/crates`; plugin
implementations, runtime scripts and system-test fixtures live under `plugins/julia`.

## Durable task and installation state

`extensions/registry.sqlite` commits the active registry, terminal task archive and installation
receipts in one SQLite transaction. The old `registry.json` is imported once when no SQLite
checkpoint exists; the original file is retained. Active task admission counts running work only:
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

Cache cleanup only removes manifest-declared relative directories while no views/tasks hold
the plugin. It preserves project results, settings outside those directories and signing identity.
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
