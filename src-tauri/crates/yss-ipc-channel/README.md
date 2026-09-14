# Desktop channel adapters

> Status: Current
> Scope: Channel encoding, subscriptions and delivery lifecycles
> Canonical owners: This crate owns Tauri stream adapters; application/domain owners retain committed state and stream facts
> Update when: Channel ordering, handoff, bounds, cancellation or termination change

| Module             | Responsibility                                                                           |
| ------------------ | ---------------------------------------------------------------------------------------- |
| `harness`          | Merge historical replay with buffered live Harness events and deliver each sequence once |
| `project_progress` | Bound progress delivery and preserve worker drain, deadline and termination behavior     |

Commands create/bind subscriptions through these adapters. All `#[tauri::command]` handlers and `invoke_handler()` remain in `yss-application::ipc`; this crate never depends on Application or Event. Shared wire types come from `yss-ipc-contract`.

Application-specific execution encoding and graph-client handoff live in [Application IPC channel adapters](../yss-application/src/ipc/channel/mod.rs). Graph preparation stays in Application use cases; the graph hub validates receipt identity and acknowledges adoption without creating a second draft store. Harness persistence and execution result authority remain in their existing owners. Structured runtime observations use the log platform plugin and its own Channels.

See [Desktop IPC](../yss-application/src/ipc/README.md) for the shared wire contract.
