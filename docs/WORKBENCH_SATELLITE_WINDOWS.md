# Workbench Satellite Windows

YssBI uses two window shells:

## Main Workbench (`EditorWindow`)

- Full VS Code-style layout: Activity Bar, Sidebar, Editor Grid, Panel, Detail, Status Bar.
- Layout persisted via `workbenchLayoutMemento`, **scoped per Tauri window label** (`setWorkbenchLayoutWindowScope` during `useWorkbenchLayout` bootstrap).
- Secondary editor windows open `#/editor`, use secondary geometry (`usePersistedSecondaryWindow`), and maintain **independent** workbench mementos — not a shared global layout.
- **Reset Layout** (View menu) restores chrome defaults only; editor grid topology, tabs, and active group are preserved.

## Presentation Windows (`PresentationWindowShell`)

Used for focused tools that are **not** workbench Parts:

| Window | Purpose |
|--------|---------|
| Plot | Chart preview |
| Database Editor | Data tables |
| Source Inspector | Resource metadata |
| Log (detached) | Log panel torn off via HTML5 DnD from embedded panel |

These windows **do not** read/write the workbench layout tree. They reuse theme/settings sync (`CLIENT_SETTINGS_UPDATED_EVENT`).

## Panel Views (`PanelPart`)

- Tab strip is driven by `panelPartModel` (`PANEL_VIEW_SPECS` + `DEFAULT_PANEL_VIEWS`). Only views with `implemented: true` appear as tabs.
- **Logs**: full implementation; detach via HTML5 DnD → `LogWindow`.
- **Output**: placeholder for future Output Channel API (build/extension stdout), distinct from Logs.
- **Terminal**: not implemented. Requires Rust PTY session + xterm.js + debounced resize via `partResizeNotifier`. To add later: set `terminal.implemented: true`, register `TerminalPanel` in `viewRegistry`, implement PTY IPC.

## Detaching Panel Views

- **Logs**: drag handle in `LogPanelContent` → standalone `LogWindow` (reference implementation).
- Future Terminal/Webview panels should follow the same pattern: embedded in `PanelPart` tab strip, optional detach to satellite shell.

## Secondary Editor Windows

Each `#/editor` Tauri webview is a **full independent workbench**:

| Concern | Scope |
|---------|--------|
| Layout chrome + editor grid | `workbenchLayoutMemento` keyed by window label (`setWorkbenchLayoutWindowScope`) |
| Window geometry | `useEditorWindowGeometryPersistence` — `main` → backend; secondary labels → `yssbi-secondary-window-*` |
| Project data / graph events | Shared via Tauri backend + `ProjectListener` per window (not layout/tabs) |
| Theme / settings | `CLIENT_SETTINGS_UPDATED_EVENT` (cross-window) |

Opening a secondary window (`Window → New Window`) does **not** clone tabs or layout from the main window. Each instance hydrates its own memento (or defaults on first open).

## Multi-window sync (deferred)

- Theme/settings sync via `CLIENT_SETTINGS_UPDATED_EVENT` is implemented.
- **Workbench layout, editor grid, and open-tab state are intentionally not synchronized** across editor windows. VS Code uses the same per-window layout model; YssBI defers any optional “mirror tabs across windows” product feature until there is a clear use case and conflict-resolution design.

## Boundaries

- **Presentation satellites** (Plot, Database Editor, Log, etc.) do **not** mount workbench `LayoutNodeRenderer` or read/write workbench layout mementos.
- **Secondary editor windows** (`#/editor`) are full per-window workbenches: each mounts `LayoutNodeRenderer`, scopes layout persistence via `setWorkbenchLayoutWindowScope`, and owns its own editor grid, tabs, and chrome Part state.
- Part resize / sash APIs apply within each editor workbench window instance (main or secondary), not across windows.
