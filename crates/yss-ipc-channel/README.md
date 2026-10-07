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

Harness subscriptions retain one sequence-ordered live queue and one active drainer per subscription.
The hub lock only protects queue admission and delivery reservation; Channel sends run after releasing it.
Reserving a sequence also rejects its duplicate while delivery is in flight. Concurrent or reentrant
publication only queues behind the active drainer, and unsubscribe prevents it from claiming further
events. A send already in flight may finish after unsubscribe. Removed subscriptions are
released outside the hub lock so a final Channel drop cannot run external cleanup under it.

Replay is consumed incrementally by the same drainer, so long persisted history does not fill the live
queue. More than 256 pending live events closes admission, drops the backlog and retains only its
highest sequence as a final gap notification. The drainer sends that notification after any in-flight
event and removes the subscription; send failure also removes it. Frontend recovery continues through
the existing durable replay contract, with no new wire event or business state.

Project progress publication uses the presence of its sender as the sole open/closed state.
Closing takes that sender under the admission lock and drops it outside the lock; nonblocking
enqueue shares the same lock, preserving the existing worker drain and deadline behavior.

See [Desktop IPC](../yss-application/src/ipc/README.md) for the shared wire contract.
