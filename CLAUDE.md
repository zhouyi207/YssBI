# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Authoritative guidance

- `AGENTS.md` is the repository's rule source. Follow it for architecture boundaries, UI behavior, diagnostics, and verification policy.
- `docs/development/LOCAL_WORKFLOW.md` is the command authority; `docs/architecture/DIAGNOSTICS_ERRORS_AND_OUTPUT.md` is authoritative for diagnostics, IPC errors, execution traces, and program output.
- `docs/architecture/ARCHITECTURE.md` contains useful product context, but parts of its backend module map and logging/execution descriptions predate the current `node_system` architecture. Verify claims against current code before relying on them.
- Preserve unrelated working-tree changes. This repository is often developed with large in-progress migrations.

## Toolchain and commands

Requirements: Node.js >= 22.22, pnpm 11.20.0, Rust >= 1.94 (edition 2024), and Julia >= 1.10 for Julia-backed operations/tests.

Run commands from the repository root. `.cargo/config.toml` puts all Cargo artifacts in root `target/`; do not `cd src-tauri` and create a second `src-tauri/target`.

```bash
pnpm install
pnpm dev                 # Vite frontend only, port 1420
pnpm tauri:dev           # desktop app
pnpm build               # frontend production build
pnpm tauri:build         # desktop installers
pnpm typecheck           # TypeScript strict/no-unused check
pnpm test                # all Vitest tests
pnpm rust:fmt:check
pnpm rust:check
pnpm rust:test:lib       # main Rust library tests, --jobs 1
pnpm rust:test           # complete main Rust crate, --jobs 1
pnpm rust:test:sci       # yss-sci tests, --jobs 1
pnpm verify              # frontend tests/typecheck + Rust fmt/check + git diff --check
pnpm verify:full         # verify plus full main-crate and yss-sci tests
```

There is no ESLint/Prettier script in `package.json`; the canonical static checks are `pnpm typecheck`, `pnpm rust:fmt:check`, and `pnpm rust:check`.

Focused tests:

```bash
pnpm test -- src/path/to/example.test.ts
pnpm test -- src/path/to/example.test.ts -t "test name"
cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib --jobs 1 test_name -- --exact --nocapture
cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --test database_test --jobs 1 test_name -- --exact --nocapture
cargo test --manifest-path src-tauri/Cargo.toml -p yss-sci --jobs 1 test_name -- --exact --nocapture
julia --project=src-tauri/julia src-tauri/julia/tests/bayes_fit_tests.jl
```

Instantiate Julia packages once when needed:

```bash
julia --project=src-tauri/julia -e 'using Pkg; Pkg.instantiate()'
```

Prefer focused Rust tests plus `pnpm rust:check`; full Rust suites are slow and are reserved for cross-cutting runtime work, release validation, or explicit requests. Use `pnpm verify` for changes spanning frontend and Rust. `pnpm verify:full` does not build installers.

## System architecture

YssBI is a Tauri 2 desktop data-analysis IDE. React renders a multi-window, Dockview-based node editor; Rust owns project state, persistence, graph compilation/execution, databases, results, and diagnostics; scientific work is split between a pure Rust crate and a restartable Julia worker.

### Frontend (`src/`)

- `src/app/main.tsx` registers application ports before rendering. `src/app/App.tsx` uses one lazy-loaded React bundle and `HashRouter` routes for project picker, editor, database, inspector, plot, info, logs, and Bayes windows.
- Dependency direction is `views` → `features/application` → `features/domain` / `features/core` / `services`; `services` wraps IPC; `shared` and `components/ui` are reusable foundations. Domain code must remain framework/service-free, and services must not import features or views.
- Ordinary Tauri invokes belong in `src/services/` and pass through `src/services/ipc/invokeCommand.ts`. Views compose application hooks and UI rather than invoking commands or implementing workflows directly.
- Rust is authoritative for persistent project/domain state. Zustand stores are frontend projections or UI/runtime state. Reads may return DTOs directly; mutations are committed by Rust and reconciled through the `project-event` listener under `features/core/sync`. Ordered/high-frequency execution data uses Tauri `Channel`, not project events.
- Dockview/Gridview is the sole authority for pane topology, sizes, active groups/panels, and serialized layout. Do not mirror placement in Zustand. Backend resource paths and Dockview `panelInstanceId` values are distinct because one resource can appear in multiple groups.
- Persisted graph/resource identities are opaque paths such as `events/...`, `functions/...`, `variables/{uuid}`, and `databases/{id}`. Never derive semantics by parsing them in frontend code.

### Tauri/Rust host (`src-tauri/src/`)

- `main.rs` calls `yssbi_lib::run()` in `lib.rs`. `lib.rs` constructs managed state, initializes diagnostics, the project registry/state, Julia worker/Bayes service, window state, plugins, and the complete Tauri command registry.
- `commands/` is the IPC adapter layer: parse/validate input, call application/domain code, map DTOs/errors, and emit events. Keep filesystem work, long workflows, and business rules out of commands.
- `application/` coordinates use cases that cross lower-level modules. `project/` owns active-project lifecycle, persistence, revisions/history, resource publication, compile caching, run registration, and `ProjectState`, the backend authority.
- `database/` and `tabular/` own source access, DuckDB/Polars data, schema/snapshots, edits, and tabular resource references. `src-tauri/sci` must not own project, DuckDB, editing-history, or UI state.

### Node system (`src-tauri/src/node_system/`)

The current graph engine is a staged system; do not reintroduce behavior from legacy `graph`/`execution` compatibility modules.

1. `protocol/` defines stable semantic node/type/port/parameter contracts.
2. `catalog/` assembles built-in providers and localized catalog projections; `registry/` validates and freezes registrations and fingerprints.
3. `document/` stores normalized, serializable graph documents and invariant-preserving mutations/history. Persisted documents contain node identities, parameters, connections, and stable dynamic-port bindings—not runtime objects or localized projections.
4. `analysis/` and `compiler/` validate, resolve dynamic interfaces/schema/types, lower the document, and publish immutable execution plans. `plan/` contains plan and execution-demand types.
5. `runtime/` is synchronous and plan-only: it consumes immutable plans and plan-local handles, never querying the registry or graph document. It owns run-scoped resources, scheduling, memoization, results, ordered output, and cancellation.

Execution enters through the frontend node-system service and `execute_graph_document`. The command streams typed events over a Tauri `Channel`; `ProjectState::execute_graph` validates project/session authority, loads function resources, reuses or compiles a plan, selects it against `ExecutionDemand`, snapshots resources, publishes function plans, and invokes `RunExecutor`. Results remain in Rust and are read through descriptor/value/page/history commands. Successful runtime resource mutations are published back as project events.

### Project persistence

A project is a directory, not one monolithic JSON file:

- `metadata.yssbi`: schema version, project name, computation settings.
- `variables.yssbi-vars`: global variables.
- `events/*.yssbi-event` and `functions/*.yssbi-function`: independently loaded/saved normalized graph resources; local variables live with their graph.
- worksheet resources under their project directory.
- `database/project.duckdb`: project-owned tables; opening a project discovers tables and rebuilds runtime database declarations.

`ProjectData` is serializable authority; `ProjectStore` contains runtime-only registries, compile/run/result state, memoization, database materialization, and traces. Preserve revision, project-instance, and publication-authority checks—stale frontend or run work must not commit into a replacement project.

### Scientific computing and Julia

- `src-tauri/src/sci/` is the application scientific boundary. Commands and node kernels call its APIs rather than depending directly on `yss_sci` or Julia internals.
- `src-tauri/sci/` is the Tauri-independent `yss-sci` Rust workspace crate for numerical/statistical algorithms.
- `src-tauri/src/julia/worker.rs` owns a single reusable, restartable Julia child process. Rust embeds assets from `src-tauri/julia/`, writes them under app data, instantiates packages, and serializes compute requests.
- Control uses newline-delimited JSON-RPC over stdin/stdout; stdout is protocol-only and diagnostics use stderr. Bulk data uses per-task Arrow IPC files (`input.arrow`, `output.arrow`) plus `metadata.json` under app data. Julia never owns project state or writes project DuckDB files.
- Edit Julia source under `src-tauri/julia/`, not the runtime copy under app data. Rust/Julia parity is checked with golden fixtures; Julia integration tests may require explicit environment flags noted in the test source.

## Error and observability boundaries

- All command failures use Rust `CommandError` with the exact wire `{ code, details, incidentId }`. Do not add backend user-facing messages, return `Result<T, String>` from commands, or parse error-string prefixes in the frontend.
- React owns localization and the feedback surface. Diagnostics are not user feedback and never drive business state.
- Rust `tracing` is the single bounded, lossy, sanitized diagnostic pipeline. Execution Trace is a separate authoritative bundle store. User Print/stdout/stderr uses the ordered bounded Run Output channel and Output panel, never diagnostics.
- Do not log dataframe rows/cells, document or clipboard contents, SQL text, connection strings, tokens, or other payload data; prefer stable IDs, codes, counts, kinds, and digests.

## Test layout

- Frontend Vitest files are colocated as `*.test.ts`/`*.test.tsx`; DOM tests opt into `// @vitest-environment happy-dom` per file.
- Main Rust unit tests are mostly module-local; integration tests live in `src-tauri/tests/`. `yss-sci` tests live under `src-tauri/sci/tests/` and module test blocks.
- Julia tests are direct scripts under `src-tauri/julia/tests/`.
- Add/run a focused regression test for behavior changes, then the relevant static/broader checks. Always run `git diff --check` before completion; do not claim verification without fresh output.
