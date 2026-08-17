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
- Command failures use the exact `{ code, details, incidentId }` wire owned by
  Rust `CommandError`; never add backend user-facing `message` fields or parse
  string prefixes. Successful response and asynchronous status DTOs must not
  smuggle backend prose through nested `message`, `detail`, or `hint` fields;
  use stable codes, safe structured fields, and incident IDs. Frontend services
  route ordinary invokes through `src/services/ipc/invokeCommand.ts`.
- Do not hold global locks during I/O, sleeps, model loading, or long-running
  inference. Take short lock snapshots and perform work outside the lock.
- Graph resources are identified by `events/...` and `functions/...` paths;
  UUIDs identify nodes, pins, connections, and UUID-backed variable resources.
  Variable resources serialize as `variables/{VariableId}` and database resources as
  `databases/{database-id}`; frontend code treats every resource path as opaque.

## Diagnostics, errors, traces, and output

- Rust `tracing` is the single diagnostic pipeline. Diagnostic storage and
  delivery are bounded, lossy, sanitized, and non-authoritative; logs never
  drive domain state, workflows, or user feedback.
- React owns application error localization and feedback presentation. Map
  stable error codes and safe details to page/section `Alert`, inline field
  errors, or a normal single-button `Dialog`; reserve `AlertDialog` for
  destructive confirmation. Rust's extensible node catalog may own deterministic
  node metadata and compiler diagnostic templates, but those templates must never
  interpolate raw internal error text.
- Do not add toaster/Sonner, `logger.notify`, browser dialogs, native message
  dialogs, disk log pagination commands, or compatibility logging paths.
- Execution Trace remains runtime-authoritative and separate from diagnostics.
  Retain and evict complete run/compilation bundles; expose truncation and drop
  counts explicitly and never repair hierarchy silently during queries.
- User-controlled Print/stdout/stderr uses the ordered bounded Run Output
  channel and Output panel, never diagnostic logs. Preserve opaque source graph
  and node identities on every output event.
- The detailed contract and capacities live in
  `docs/architecture/DIAGNOSTICS_ERRORS_AND_OUTPUT.md`.

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

- Use shadcn/ui primitives for ordinary interactive controls. Dockview is the
  specialized workbench/editor docking infrastructure; do not introduce another
  general-purpose UI component library.
- Use persistent shadcn `Alert` surfaces for page/section errors, inline
  feedback for field errors, and the application `MessageDialog` for blocking
  acknowledgement. Never use toaster/Sonner, browser `alert`, `prompt`,
  `confirm`, or native message dialogs; path selection dialogs are the only
  exception.
- User-facing scrolling uses `src/components/ui/scroll-area.tsx`; preserve the
  surrounding `flex`, `min-h-0`, and `flex-1` layout contract.
- Context menus use compact spacing (`py-0`, no separator margins) and small
  outer radii (`rounded-sm`/`rounded-md`).
- Centralize drag/drop constants, payload types, and guards in
  `src/features/core/dnd/`; workspace routes drops and visual zones stay thin.

## Workbench and editor layout

- Dockview is the sole authority for workbench pane topology/sizes, editor group
  topology/sizes, panel placement/order, active group/panel, and serialized
  layout restoration. Do not mirror those values into Zustand or recreate a
  `LayoutTree` compatibility model.
- `GridviewReact` owns the outer sidebar/editor-shell/detail panes. The shell
  `DockviewReact` hosts the editor surface in its center and Logs/Output in a
  native edge group; the nested editor `DockviewReact` owns editor groups and
  panels. Keep menubar, activity bar, status bar, and modals outside the Dockview
  workspace.
- The shell edge group owns panel placement, splitter size, collapsed state, and
  last expanded size. Collapse only the edge-group content while retaining its
  tab bar; never emulate collapse by resizing an outer Gridview leaf to the
  header height.
- Application stores may keep non-placement panel metadata and pane-local state
  keyed by `panelInstanceId`; resource identity remains separate and opaque.
- Route all panel closes through application dirty/save confirmation before
  removing the Dockview panel. Floating groups and browser popouts stay disabled;
  Tauri owns application windows.

## Graph lifecycle and synchronization

- On project load, clear backend state before hydrating it. On graph open,
  load the graph, bind runtime services, materialize dynamic pins, then hydrate
  the frontend projection.
- Writes go through backend commands and return through project events. If a
  UI action is optimistic, track pending keys and suppress only its matching
  echo.
- Graph resources and all IPC references use opaque `graphPath` values. Dockview
  panel IDs are separate `panelInstanceId` values because one resource may be
  open in multiple groups; panel metadata carries the resource reference. Do not
  infer backend-loaded state from `graphEntities` alone; use the graph session
  and resource loaded state.
- Save/close flows use the application confirmation modal and keep sub-windows
  read-only with respect to project state.

## Test scope discipline

- Add the smallest focused regression test that proves the requested behavior
  or reproduces the reported bug. Prefer one representative case per distinct
  project-owned contract or failure mode.
- Test through stable public seams rather than private implementation details.
  Do not add tests at unrelated or unconfirmed seams.
- Do not add speculative edge-case tests, exhaustive input matrices, duplicate
  coverage, or tests for unchanged behavior unless acceptance criteria require
  them or a demonstrated regression risk justifies them.
- Before adding more than two new tests, identify the distinct regression each
  additional test catches; omit cases without a unique failure mode.
- For behavior-preserving refactors, rely on existing coverage and run the
  relevant suites. Add tests only when current coverage cannot establish the
  preserved contract.
- Do not test behavior owned by the language, framework, type system, or third-
  party dependencies; test only contracts owned by this project.

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
- Use `pnpm verify` before delivery when changes span both frontend and Rust. It
  runs the frontend suite plus Rust format/compile checks; run focused Rust tests
  separately for the touched area. Use `pnpm verify:full` only for release
  validation, cross-cutting runtime changes, or when explicitly requested. Neither
  command builds a release installer.
- Do not claim success without fresh command output. Preserve unrelated user
  changes in the working tree.
