# YssBI Engineering Guide

This file is the single project rule source for Codex agents. Keep project
behavior and architecture consistent with these rules; update this file when
the architecture changes.

## Architecture and boundaries

- Keep responsibilities narrow, cohesion high, and dependencies flowing one way.
- Split modules by responsibility or feature boundary; avoid god modules,
  circular dependencies, and duplicate compatibility paths.
- Preserve behavior during refactors unless the task explicitly requests a
  behavior change. Remove deprecated or legacy shims directly in this 0.x
  project instead of adding migration layers.
- Keep Rust and React separated. Rust owns domain state and business logic;
  React stores are projections and UI state.
- Prefer focused functions (roughly 50 lines) and focused files (roughly 300
  lines); split larger units when it makes a real boundary clearer.

## Rust and Tauri

- `ProjectState.project_data` is the authoritative project/graph/pin state.
- `ProjectState::insert_graph` is the only graph insertion path; loading,
  creation, duplication, import, and restore must preserve its runtime setup.
- Tauri commands live under `src-tauri/src/commands/` and stay thin:
  parse/validate input, call domain/application code, map to DTOs, and emit
  events. Do not put long workflows, filesystem I/O, or duplicated validation
  in commands.
- Use serializable DTOs at IPC boundaries. Frontend invokes belong in
  `src/services/`; views must not call `invoke` directly.
- Use commands for request/response operations, events for low-rate state
  changes, and channels/workers for ordered or high-frequency streams.
- Do not hold global locks during I/O, sleeps, model loading, or long-running
  inference. Take short lock snapshots and perform work outside the lock.
- Graph resources are identified by `events/...` and `functions/...` paths;
  UUIDs identify nodes, pins, and connections only.

## React organization

- `views/` composes screens; `features/application/` coordinates use cases;
  `features/domain/` contains pure types/functions; `features/core/` contains
  shared infrastructure; `services/` wraps IPC/API calls; `components/` is
  domain-agnostic UI.
- Domain code must not import UI, services, or framework state. Services must
  not import views or features.
- Subscribe to Zustand stores with narrow selectors. Keep stores domain-scoped;
  put complex workflows in application hooks/services.
- Persist cross-session preferences in `localStorage`; keep temporary UI and
  runtime state in Zustand. Backend state remains backend-authoritative.
- Use React Router for navigable pages and route state, not active-page flags.
- Route all global `window`/`document` listeners through
  `src/shared/utils/globalEvent.ts`.

## UI and interaction

- Use shadcn/ui primitives for interactive controls; do not introduce a second
  UI component library.
- Use the shared toast system in the bottom-right for ordinary messages.
  Never use browser `alert`, `prompt`, `confirm`, or native message dialogs;
  path selection dialogs are the only exception.
- User-facing vertical scrolling uses `src/shared/ui/OverlayScrollbar.tsx`;
  preserve the surrounding `flex`, `min-h-0`, and `flex-1` layout contract.
- Context menus use compact spacing (`py-0`, no separator margins) and small
  outer radii (`rounded-sm`/`rounded-md`).
- Centralize drag/drop constants, payload types, and guards in
  `src/features/core/dnd/`; workspace routes drops and visual zones stay thin.

## Graph lifecycle and synchronization

- On project load, clear backend state before hydrating it. On graph open,
  load the graph, bind runtime services, materialize dynamic pins, then hydrate
  the frontend projection.
- Writes go through backend commands and return through project events. If a
  UI action is optimistic, track pending keys and suppress only its matching
  echo.
- Graph tab IDs and all IPC/store references use `graphPath`. Do not infer
  backend-loaded state from `graphEntities` alone; use the graph session and
  resource loaded state.
- Save/close flows use the application confirmation modal and keep sub-windows
  read-only with respect to project state.

## Local development and verification

- Run project commands from the repository root through `pnpm` scripts. Rust
  scripts explicitly target `src-tauri/Cargo.toml`; `.cargo/config.toml` keeps
  all Cargo artifacts in the root `target/` directory. Do not create a second
  `src-tauri/target/` through ad-hoc Cargo invocations.
- The canonical workflow and command matrix are documented in
  `docs/development/LOCAL_WORKFLOW.md`. Do not add CI as a substitute for these
  local checks unless the task explicitly requests it.
- Add or update a focused regression test before changing behavior, then run it
  and the relevant broader test suite.
- For Rust changes, run `pnpm rust:check` and focused tests for the touched
  area. Do not run full `cargo test` / full `pnpm rust:test` by default because
  it is slow; only run broader Rust suites when the change is cross-cutting,
  explicitly requested, or the focused checks cannot cover the risk. For
  frontend changes, run `pnpm typecheck` and the relevant `pnpm test` coverage.
  Run `git diff --check` before reporting completion.
- Use `pnpm verify` before delivery when changes span both frontend and Rust;
  it is a local verification command and does not build a release installer.
- Do not claim success without fresh command output. Preserve unrelated user
  changes in the working tree.
