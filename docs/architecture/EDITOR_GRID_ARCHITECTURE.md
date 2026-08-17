# Editor Workbench Layout Architecture

## Authority model

YssBI uses the live Gridview/Dockview instances as the only layout authority. There is no parallel application-owned topology model.

| Layer | React host | Owns | Application adapter |
|---|---|---|---|
| Outer workbench | `GridviewReact` in `Workspace.tsx` | Sidebar, editor shell, and detail topology, visibility, and splitter sizes | `workbenchGridPort` |
| Shell dock | `DockviewReact` in `PanelPart.tsx` | Center editor host plus the Logs/Output edge group, including edge position, splitter size, active panel, and collapsed state | `panelDockviewPort` |
| Editor dock | Nested `DockviewReact` in `Workspace.tsx` | Editor groups, panels, tab order, active group/panel, and editor split topology | `editorDockviewPort` |

`useWorkbenchStore` contains only non-placement UI preferences and temporary state such as the selected sidebar tab, user visibility intent, modal state, and Zen mode. It must not mirror Gridview/Dockview topology, sizes, active state, or edge-group collapse.

## Rendered hierarchy

```text
EditorWindow
├─ Menubar
├─ horizontal body
│  ├─ ActivityBar
│  └─ Workspace
│     └─ GridviewReact
│        ├─ Sidebar
│        ├─ editor shell
│        │  └─ DockviewReact
│        │     ├─ center EditorHost
│        │     │  └─ DockviewReact
│        │     │     └─ editor groups and panels
│        │     └─ edge group
│        │        ├─ Logs
│        │        └─ Output
│        └─ Detail
└─ BottomBar
```

Menubar, activity bar, status bar, dialogs, and modal overlays remain outside the Gridview/Dockview workspace. Floating groups and browser popouts are disabled; restored Dockview layouts are sanitized before application.

## Module seams

- `src/features/core/workbench/workbenchGridPort.ts` adapts the outer `GridviewApi`. It exposes serialization, restoration, reset, and part visibility without copying the topology.
- `src/features/core/dockview/panelDockviewPort.ts` adapts the shell `DockviewApi`. It owns panel activation, edge-group placement, collapse/expand, restoration, and serialization.
- `src/features/core/dockview/dockviewEditorPort.ts` adapts the nested editor `DockviewApi`. It owns open, activate, update, close, move, split, resource remap, restoration, and serialization operations.
- `src/features/core/dockview/types.ts` is the shared serializable interface for editor panel identity, resource metadata, and split requests.
- `src/features/core/dockview/editorPaneStateStore.ts` stores pane-local projections keyed by `panelInstanceId`; it does not own panel placement or resource state.
- `src/features/application/editor/` coordinates dirty/save confirmation, graph session lifecycle, and calls into the Dockview seam.

Views do not maintain a second group tree. Application code observes layout changes through the ports and sends mutations through their focused interfaces.

## Identity and editor state

Three identities remain distinct:

- `resourceRef` is the opaque backend resource path, for example `events/...` or `functions/...`.
- `panelInstanceId` identifies one Dockview panel instance. Multiple panels may show the same resource.
- `groupId` is owned by Dockview and identifies the current editor group.

Panel metadata carries the resource reference. Code must not derive a resource path from a panel or group id, and frontend graph projections must not be treated as proof that a backend graph session is loaded.

Dockview owns active group/panel and tab ordering. A Dockview activation is projected into the editor application flow, while application-initiated activation goes through `editorDockviewPort`. Panel close requests first pass through application dirty/save confirmation and only then call the Dockview close path.

## Splitting and drag/drop

Editor splits use Dockview directions (`top`, `bottom`, `left`, `right`) and `editorDockviewPort.split()`. Programmatic moves use `editorDockviewPort.move()`; native Dockview tab drag/drop remains authoritative for topology changes it performs.

Sidebar graph-resource drops route through the application editor workflow and then request a Dockview split when a direction is present. Workspace drag payloads and guards stay centralized under `src/features/core/dnd/`.

The editor and shell docks disable floating groups. Tauri, not Dockview, owns application windows.

## Shell edge group

Logs and Output live in one native Dockview edge group. That edge group owns:

- bottom/left/right placement;
- splitter and expanded size;
- active Logs/Output tab;
- collapsed state and collapsed header height.

Collapsing the panel collapses only edge-group content and retains its tab bar. The outer Gridview editor leaf is not resized to emulate a collapsed panel. `panelDockviewPort` exposes the live collapsed snapshot and preserves the expanded size when moving or repeatedly collapsing the edge group; Zustand does not mirror that state.

## Persistence and restoration

`src/features/core/dockview/dockviewLayoutPersistence.ts` stores one window-scoped value under `yssbi-dockview-layout:<window-label>`:

```text
{
  workbench: SerializedGridviewComponent,
  shell: SerializedDockview,
  editor: SerializedDockview,
  preferences: non-placement workbench UI preferences
}
```

Persistence serializes each live authority directly. Edge-group collapse is serialized only in the shell Dockview layout, never in `preferences`. Hydration restores the outer Gridview first, waits for the shell Dockview, restores the shell, waits for the nested editor Dockview, and finally restores the editor layout. A hydration generation guard prevents stale asynchronous restoration from applying preferences after reset.

There is no alternate topology schema or compatibility reconstruction path. Reset restores the authorities' captured defaults and resets only the related non-layout UI preferences.

## Public-seam verification

Layout behavior is verified through the interfaces used by production callers:

- `workbenchGridPort.test.ts` covers the outer Gridview adapter.
- `dockviewEditorPort.test.ts` covers panel/resource identity, queued operations, events, restore sanitization, and close behavior.
- `panelDockviewPort.test.ts` covers shell edge-group placement and collapse behavior.
- `dockviewLayoutPersistence.test.ts` covers coordinated restoration and stale-hydration invalidation.
- application Dockview tests cover synchronization at editor workflows rather than private layout helpers.
