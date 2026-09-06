# yss-sci-contract

Backend-neutral scientific-computing contracts shared by application workflows,
scientific runtimes, and concrete backend adapters.

This crate owns statistical input values, computation settings, monotonic
execution controls, cancellation tokens, and stable scientific error codes. It
does not own algorithms, project/database state, Julia processes, Tauri
transport, or presentation DTOs.

Statistical observation metadata records row selection counts and the applied
missing-value policy. It carries no project/node setting provenance or unused
convergence-tolerance fields.
