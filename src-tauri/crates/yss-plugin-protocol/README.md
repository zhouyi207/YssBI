# Plugin protocol

Version 0.2 of the shared contract implements wire protocol major 2. The package owns manifests,
budgets, task states, diagnostics and operation identity validation; it has no host implementation
dependency. Rust schemas generate host transport types, while the Web SDK carries the same protocol.

`scripts/generate-plugin-contract.mjs` owns frontend type/schema generation beside this crate.
Run `pnpm generate:plugins` or `pnpm generate:plugins:check` from the repository root.

Operations use a creation timestamp and a nonce. Reuse the identifier and parameters for retries;
the host retains receipts for 30 days and rejects expired identifiers instead of executing them again.
Granted budgets are host-owned context facts. Snapshot/result limits are enforced at data boundaries;
private storage for trusted native processes is an application soft limit, not an OS sandbox.

`outcomeUnknown` is a terminal observation of uncertain execution. It must remain distinct from
`failed`; clients must not automatically turn it into a new operation.
