# Shared desktop IPC contract

> Status: Current
> Scope: Shared wire DTOs, event envelopes and command error payloads
> Canonical owners: This crate owns shared serialization; adapters own conversion from application/runtime facts
> Update when: A shared IPC payload changes

`yss-ipc-contract` defines project receipts, graph projections, Harness messages, execution stream values, project progress and `CommandErrorDto`. Command, Event and Channel consume these same types.

The crate has no Tauri, Application, database or execution runtime dependency. It reuses neutral identity/document contracts and existing projection value types, including their value conversions. It creates no subscriptions, tasks, caches, incident records or business state. Application mappings and incident recording remain in their adapters.

Command-only request/response schemas may stay beside their handler. Shared types have one definition here, with no compatibility re-export from the former schema modules.

The complete wire and delivery contract is maintained in [Desktop IPC](../yss-application/src/ipc/README.md).
