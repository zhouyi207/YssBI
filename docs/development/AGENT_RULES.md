# YssBI Repository Agent Policy

> Status: Current
> Scope: Repository-wide coding-agent behavior and cross-system guardrails
> Canonical owners: This file owns change discipline and cross-system guardrails; the root `.rules` owns instruction loading, scope and validation discipline
> Update when: Repository-wide agent policy or cross-system guardrails change

This policy applies throughout the repository, regardless of its location
under `docs/`.

Unless stated otherwise, repository paths in this policy are relative to
the repository root.

## Scope and precedence

Use repository knowledge in this order:

1. Code, tests, and manifests own executable facts such as versions, paths,
   dependencies, constants, and command registration.
2. Module `README.md` files own their current responsibilities and stable contracts;
   `docs/architecture/` owns only the system overview and cross-module relationships.
3. `.rules` owns coding-agent behavior and cross-system guardrails.
4. `docs/development/` owns change, validation, and delivery workflows.
5. `docs/decisions/` explains accepted design choices.
6. `docs/roadmap/` contains plans, remaining acceptance work, and explicitly
   checked completion summaries. These records do not replace current contracts.
   Git retains obsolete design discussions, review snapshots, and version history.

When maintained documentation and code disagree, inspect the implementation and
tests, then update the stale canonical document in the same change. Never infer
current behavior from a roadmap or version note.

Files under `docs/superpowers/` are temporary agent-local planning output. Do not
add or commit them unless explicitly requested.

## Change discipline

- Keep work scoped to the requested outcome and preserve unrelated user changes.
- Prefer changing the existing owner of a responsibility. Add a module only for
  a distinct responsibility or a real boundary.
- When reusing an existing implementation can simplify code without adding an
  abstraction layer, proactively notify the user and briefly explain what can
  be reused and how it simplifies the code.
- Preserve observable behavior during refactors unless the request changes it.
- This is a 0.x project: remove obsolete internal paths directly unless
  compatibility is an explicit requirement.
- Remove unused interface parameters and update all callers directly; do not retain
  placeholder parameters for possible future use.
- Do not add parallel models, compatibility facades, speculative abstractions,
  dependencies, or features merely to make a local edit easier.
- Comments should explain non-obvious reasoning, invariants, or trade-offs, not
  restate the code.
- `TODO.md` contains open work only. Do not append completed change summaries;
  Completed items may be summarized with `- [x]` in the relevant release roadmap;
  Git retains detailed implementation history. Keep unfinished acceptance items open.
- When a change alters current architecture, a public contract, or the
  development workflow, update its canonical document in the same change.

## Architecture invariants

- Rust is the authority for committed project state, persistence, graph
  analysis, execution planning, databases, results, and scientific orchestration.
- Project calendar values and user-facing timestamps are timezone-free. Removing
  an input timezone must preserve its original calendar and wall-clock fields.
- Rust Project owns the current GraphDocument, graph undo/redo and saved-content
  identity. React keeps graph read projections and transient interaction state;
  do not introduce a separate Graph draft authority. Other resource-specific
  frontend drafts retain their explicitly documented owner.
- Do not merge or reconcile parallel committed Rust and React models. Replace
  graph projections in one direction. GUI edits, history navigation and execution
  do not implicitly save the graph body. Explicit Save persists the current graph;
  Assistant edit batches persist their complete current graph atomically and retain
  undo history, without requiring an editor panel.
- Dependencies flow toward domain and application logic, never from domain code
  toward UI, Tauri, services, or concrete business infrastructure adapters.
  Generic filesystem primitives are an explicit foundation for Project and Application.
- `yss-filesystem` owns generic file access, transactions, change facts and watcher
  sessions, with zero dependencies on repository crates. Project path conventions,
  document validation, index invalidation and business failures belong to Project
  or its application adapters, never to FS.
- `yss-node-kernel` owns graph-independent invocation values, kernel contracts,
  the frozen kernel registry and built-in node adapters. It does not depend on
  Graph, Project, Application or Tauri; resource authorization, graph addresses,
  execution plans, result storage and graph error locations stay with their owners.
  Application composes one frozen registry shared by readiness checks and execution.
- Only `yss-node-kernel`, `yss-graph-execution` result analyses (and its focused SCI benchmark), and Application's
  `ipc/commands` modules consume `yss-sci-runtime`. Other Application modules and desktop composition do not
  construct or inject a scientific backend. Runtime exposes stateless functions and calls `yss-sci`, which calls
  `yss-sci-linalg`. Only Linalg depends on faer and owns matrix/vector wrappers;
  runtime uses neutral inputs/results. Julia plugin crates own their Bayes
  input and cancellation contracts and do not depend on host SCI crates.
- Desktop commands and application-specific channel adapters belong to
  `yss-application::ipc`. Its command module owns the application invoke registry and
  consumes `yss-ipc-event`, neutral `yss-ipc-channel` adapters, and shared
  `yss-ipc-contract`. Event/Channel never depend on Application; Contract has no
  Tauri or runtime dependency. Platform plugins own their namespaced commands;
  `tauri-plugin-tracing` owns structured runtime observations, log persistence and
  subscriptions. Rust producers use tracing; frontend producers use the log service.
  Logging implementation belongs inside the plugin. Graph Problems, model diagnostics,
  run state and results remain domain-owned facts, independent of log delivery.
  Commands remain thin adapters; business workflows
  belong to application use cases or domain owners.
- Command failures use the exact Rust-owned `{ code, details, incidentId }`
  wire. Rust does not send user-facing error prose; React localizes stable codes.
- `GraphSemanticSnapshot` is the only authority for resolved graph types,
  schemas, lineage, diagnostics, coercions, and kernel specialization.
- Analysis Graphs model data ports and data dependencies only. Control flow,
  effects, sequencing, and user-program side effects belong to Workflow owners.
- Graph edits resolve types, diagnostics and result validity. Save commits the
  current graph independently. Execute captures its document and semantic identity,
  prepares a matching immutable plan internally, and never implicitly saves.
  Plan caches are backend implementation details, not a separate frontend lifecycle.
- The root FlexLayout Model instance is the sole authority for workbench topology,
  placement, ordering, active panels/groups, edge sizes, and collapse state.
- Graph Problems, operational Logs, Results, and run state/failures are distinct data
  flows. None may be used to reconstruct or substitute for another.

## Testing and validation

Validation levels, scope selection and delivery evidence are owned by the root
[`.rules`](../../.rules). Concrete commands and module-specific prerequisites
belong to the corresponding module README; `package.json` owns executable scripts.

## Documentation routing

Before changing a subsystem, read its local README and applicable `.rules`.
Use `docs/README.md` for the module index. Representative owners are:

- Graph, projection, plan preparation, execution, Results, and run state/failures:
  `src-tauri/crates/yss-application/src/graph/README.md`
- Workbench layout and panel lifecycle:
  `src/modules/workbench/README.md`
- Logging delivery and storage: `src-tauri/crates/tauri-plugin-tracing/README.md`
- Operational observations, feedback, and signal boundaries:
  `src/features/application/observability/README.md`
- Statistical Harness current implementation:
  `src-tauri/crates/yss-harness-core/README.md`
- JSON pages and UI intents: `src-tauri/crates/yss-ui-contract/README.md`
- Plugin target contract: `plugins/README.md`; current behavior remains documented
  by each plugin and the host protocol/runtime READMEs.
- Tauri/IPC transport contracts: `src-tauri/crates/yss-application/src/ipc/README.md`
- Architecture review and documentation checks: `docs/development/ARCHITECTURE_GATES.md`
- Validation discipline: root `.rules`; command usage: root `README.md`,
  `src/README.md`, `src-tauri/README.md` and the affected module README.
- Feature, fix, refactor, and behavior changes:
  `docs/development/CHANGE_PROCESS.md`

Do not create generic language/framework manuals such as `ts.md`, `rust.md`, or
`tauri.md`. Put module details in the existing owner's README, and scoped editing
constraints in its `.rules`; keep system documentation short and link to these owners.
