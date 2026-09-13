# Desktop event delivery

> Status: Current
> Scope: Tauri Event delivery
> Canonical owners: This crate owns event emission; `yss-ipc-contract` owns envelopes
> Update when: Event targeting or delivery semantics change

`yss-ipc-event` sends the existing `project-event` envelope after the authoritative operation commits. Callers choose whether a delivery error is returned or recorded. Delivery failure never rolls back an already committed operation.

This crate depends on Contract and Tauri, registers no commands and owns no business state or event ledger. Event envelope definitions live in `yss-ipc-contract::event`.

See [Desktop IPC](../yss-ipc-command/README.md) for shared wire and recovery semantics.
