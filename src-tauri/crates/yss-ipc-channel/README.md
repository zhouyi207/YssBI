# Desktop channel adapters

> Status: Current
> Scope: Channel encoding, subscriptions and delivery lifecycles
> Canonical owners: This crate owns Tauri stream adapters; application/domain owners retain committed state and stream facts
> Update when: Channel ordering, handoff, bounds, cancellation or termination change

| Module             | Responsibility                                                                                            |
| ------------------ | --------------------------------------------------------------------------------------------------------- |
| `harness`          | Merge historical replay with buffered live Harness events and deliver each sequence once                  |
| `harness_graph`    | Bind the frontend draft owner, associate requests and retain Rust receipts until adoption or interruption |
| `execution`        | Encode execution and Run Output facts and deliver them through the existing channel                       |
| `project_progress` | Bound progress delivery and preserve worker drain, deadline and termination behavior                      |
| `diagnostics`      | Connect the existing neutral `DiagnosticsRuntime::subscribe_batches` sink to a Tauri channel              |

Commands create/bind subscriptions through these adapters. All `#[tauri::command]` handlers and `invoke_handler()` remain in `yss-ipc-command`; this crate never depends on Command or Event. Shared wire types come from `yss-ipc-contract`.

Graph preparation still runs in Application. The graph hub validates receipt identity and acknowledges frontend adoption; it does not execute business operations or create a second graph draft store. Harness persistence, execution result authority and diagnostics history remain in their existing owners.

See [Desktop IPC](../yss-ipc-command/README.md) for the shared wire contract.
