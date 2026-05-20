# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

```bash
npm run dev      # Start Vite dev server (port 1420)
npm run build    # Production build
npm run tauri    # Tauri CLI (e.g., npm run tauri -- dev, npm run tauri -- build)
```

No test runner or linter is configured yet.

## Tech Stack

- **Desktop shell**: Tauri v2 (Rust backend + React frontend)
- **Frontend**: React 19, TypeScript 5.8, Vite 7, Tailwind CSS v4, shadcn/ui (Radix), react-router v7 (HashRouter)
- **State management**: Zustand
- **Canvas/graph editor**: Custom SVG-based node-graph editor with @dnd-kit
- **Data tables**: @glideapps/glide-data-grid with @tanstack/react-virtual
- **Charts**: D3.js v7
- **Math rendering**: KaTeX, react-markdown, remark-math, rehype-katex
- **i18n**: i18next + react-i18next (zh-CN, en-US)
- **Rust dependencies**: polars (DataFrame), ndarray, sqlx, calamine (Excel), faer (linear algebra), statrs

## Architecture: CQRS + Backend-Authoritative State

The app follows a **command-query separation** pattern where the Rust backend (`src-tauri/src/`) is the single source of truth.

### Data Flow

```
UI → Hooks → Services (invoke) → Rust Commands → State → emit event → Frontend Sync Layer → Zustand Stores → UI
```

- **Queries** (reads): `invoke` returns data directly, used for one-off fetches
- **Mutations** (writes): `invoke` mutates backend state, backend emits a `project-event`, frontend sync handlers in `src/features/core/sync/handlers/` update Zustand stores

### Frontend Layer Architecture (`src/`)

```
views/              # Pure presentational pages — consume hooks from features/application/
features/
  application/      # Use-case orchestration hooks that coordinate domain + services
  domain/           # Pure business logic, types, value objects — zero side-effects
  core/             # Shared infrastructure: stores, sync engine, history, DnD contracts
services/           # Thin wrappers around Tauri invoke() — no UI or store logic
components/ui/      # shadcn/ui primitives (Button, Dialog, Select, etc.)
shared/             # Cross-cutting types (domain, DTO, settings, UI), reusable UI widgets
app/                # Entry point (main.tsx, App.tsx), i18n init, global providers
```

**Dependency direction**: `views → features/application → features/domain | services | features/core`. No reverse dependencies.

### Key Stores (all Zustand, in `src/features/core/`)

| Store | Purpose |
|---|---|
| `graphMetaStore` | Graph metadata (name, type, folderPath) — the project index |
| `graphDataStore` | Graph body (nodes, pins, connections) with entity + index tables |
| `projectIOStore` | Project load/save lifecycle, coordinates hydration across all stores |
| `variableStore` | Global variables |
| `databaseStore` / `columnStatsStore` / `columnDistributionStore` / `datasetOverviewStore` | Database/DataFrame metadata and statistics |
| `useEditorStore` | Active editor tab state, selection, clipboard |
| `historyStore` | Per-graph undo/redo stacks (command-based, not snapshot-based) |
| `layoutStore` | Window layout, tabs, panels, groups |
| `useViewportStore` | Canvas viewport (pan/zoom) per graph |
| `sidebarStore` / `sidebarDragStore` | Sidebar state and drag initiation |
| `UIStore` | Toast messages, modals, loading progress |

### Sync Layer (`src/features/core/sync/`)

- `ProjectListener` — global singleton that listens to Tauri `project-event` and dispatches to typed handlers
- `EventRegistry` — maps event types to handler functions
- Handlers directly update Zustand stores; optional callbacks (e.g., open a new tab) flow through `useProjectSync`
- `echoSuppressor.ts` — `trackPending`/`isPending` to prevent self-echo on optimistic updates (e.g., node drag)

### Rust Backend Structure (`src-tauri/src/`)

```
commands/           # All #[tauri::command] — thin wrappers, parse args, delegate to modules
project/            # ProjectState, ProjectRegistry, load/save lifecycle
graph/              # Graph CRUD, node registry, schema propagation
execution/          # Graph execution engine
database/           # Database engines (SQLite, PostgreSQL, MySQL, Excel)
event/              # Event emission types
schema/             # Node definitions and editor schema
editor/             # Editor-level operations
sci/                # Scientific computing crate (regression, stats, panel, time series)
```

**Rule**: `#[tauri::command]` only in `commands/`. Business logic lives in domain modules. `ProjectState::insert_graph()` is the **only** entry point for adding graphs to project state — it binds the runtime (registry, schema provider, pin resolution).

## Conventions

- **All UI must use shadcn/ui primitives** from `@/components/ui/`. No raw HTML buttons/inputs/dialogs. No competing UI libraries (MUI, Ant Design, etc.).
- **No native dialogs** (`window.alert`, `window.confirm`, `window.prompt`). Use the React modal system (`uiStore.confirm` / `uiStore.confirm3`) or toast. The only exception is file/directory pickers via `@tauri-apps/plugin-dialog`.
- **Toast position**: bottom-right, via shadcn toast (`sonner`).
- **No compatibility shims** — the project is pre-release. Replace wrong formats/APIs directly instead of preserving legacy.
- **Zustand subscriptions must use selectors**: `useStore(s => s.field)`, never destructure the whole store.
- **DnD contracts** are centralized in `src/features/core/dnd/`. No raw drag/drop type strings scattered in components.
- **Global event listeners** on `window`/`document` must use `src/shared/utils/globalEvent.ts` (`addGlobalEventListener`) for consistent cleanup.
- **IPC**: Frontend never calls `invoke` directly from views — always go through `services/`. Commands use camelCase JSON.
- **Files**: Aim for ≤300 lines per file, split when exceeding 500.

## Routes

All routes use HashRouter. Key routes: `/` (project picker), `/editor` (main editor window), `/plot`, `/dataview`, `/logs`, `/info`.

## i18n

Translation keys use English as the key string. Add new keys to both `src/app/i18n/locales/zh-CN.ts` and `en-US.ts`. Use `useTranslation()` hook in components.
