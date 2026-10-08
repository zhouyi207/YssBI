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

Package, executable, view and storage paths use the same portable relative-path predicate.
It rejects reserved filename punctuation, control bytes, device names and trailing dots/spaces.
Cache directory overlap uses ASCII case-insensitive identity and directory boundaries while
preserving the declared spelling. A parent and child directory cannot both be cleanup roots.

Run `cargo run -p yss-plugin-protocol --bin plugin-schema` from the repository root for schema output.

Custom views are signed `.view.json` assets described by `NativeView`. The contract owns bounded
text, number, boolean, choice and JSON inputs and declared command/task/view actions. Validation
checks unique control IDs, initial values, byte/count limits and manifest method/declaration grants.
Task actions also require `tasks.get` so the host can observe their result without retrying work.
`ViewSession.view` carries this model; command replies use `NativeCommandReply` and may replace
the form. HTML, scripts and unknown wire fields are rejected; no legacy-format conversion exists.

Operations use a creation timestamp and a nonce. Reuse the identifier and parameters for retries;
the host retains receipts for 30 days and rejects expired identifiers instead of executing them again.
Granted budgets are host-owned context facts. Snapshot/result limits are enforced at data boundaries;
private storage for trusted native processes is an application soft limit, not an OS sandbox.

`outcomeUnknown` is a terminal observation of uncertain execution. It must remain distinct from
`failed`; clients must not automatically turn it into a new operation.
