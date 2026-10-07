# Plugin protocol

> Status: Current
> Scope: 插件清单、协议类型、预算、任务身份与 schema 生成
> Canonical owners: 本 crate 类型与生成器拥有当前 wire
> Update when: 协议、清单、schema 或生成入口改变时

Version 0.2 of the shared contract implements wire protocol major 2. The package owns manifests,
budgets, task states, diagnostics and operation identity validation; it has no host implementation
dependency. The `plugin-schema` binary derives JSON Schema directly from the same Rust types.

Manifest validation rejects duplicate IDs within each view, command and task-type collection.
A task ID therefore selects one artifact-production rule before the host applies project and
result-write admission checks; declaration order cannot select between conflicting rules.

Run `cargo run -p yss-plugin-protocol --bin plugin-schema` from the repository root for schema output.
`scripts/generate-plugin-contract.mjs` retains generation for the archived React contract reference;
it is outside the native build.

Operations use a creation timestamp and a nonce. Reuse the identifier and parameters for retries;
the host retains receipts for 30 days and rejects expired identifiers instead of executing them again.
Granted budgets are host-owned context facts. Snapshot/result limits are enforced at data boundaries;
private storage for trusted native processes is an application soft limit, not an OS sandbox.

`outcomeUnknown` is a terminal observation of uncertain execution. It must remain distinct from
`failed`; clients must not automatically turn it into a new operation.
