# Editor Application Layer

Editor UI orchestration lives under `features/application/editor/`. Core editor state and actions live under `features/core/editor/`.

## Structure

```
features/application/editor/
├── editorCanvasTypes.ts       # Panel-scoped Canvas contracts
├── workbenchCommandCapability.ts # Explicit Workbench command contract
├── useEditorCanvas.ts         # Panel-scoped Canvas commands/workspace/interaction
├── useCanvasInteraction.ts    # Gesture cancellation, currentness, and palette/reroute actions
├── useCanvasViewport.ts       # Shared viewport session adapter
├── useEditorOperations.ts     # Clipboard, history, node ops
├── useEditorPanelCommands.ts  # Narrow panel open/split facade
└── index.ts

features/core/editor/
├── hooks/                     # Narrow projection and UI hooks
└── index.ts
```

## Usage

### Workbench composition

```tsx
const commands = useWorkbenchCommandCoordinator();

<WorkbenchWindow commands={commands} />;
```

Only `app/windows/workbench/integrations/workbenchCommandCoordinator.ts` combines menu, keyboard,
project, graph, and chart commands. Views receive the typed capability through props.

### Canvas

`GraphCanvasController.tsx` is the sole caller of the panel-scoped hook:

```tsx
const canvas = useEditorCanvas({
  mode: "interactive",
  scope: { panelInstanceId, groupId, graphPath, graphKind },
});
```

Use `mode: 'preview'` for an inactive or saving Dockview panel. Preview disables
editing, selection, zoom/pan, and context-menu actions; its sidebar drop route remains
registered so the workbench can activate the target before dropping.

The graph-editor module mounts one React Flow provider per panel/group/resource.
Its nodes and edges are derived from the editor projection, not saved graph documents.
Node/port views remain custom React components; dynamic handle IDs use the existing port keys.
Controlled nodes retain measured dimensions and dragging flags, and unchanged nodes keep their
references. Measurements are accepted in preview mode as well; losing them would hide nodes
and reset connection anchors on every update.
`useCanvasInteraction` grants a current, cancellable gesture; the renderer keeps its
position previews local and submits one existing mutation at gesture end. Escape,
deactivation, graph close, and project replacement invalidate the gesture.
Late mutation completion only clears that mutation's preview, never a newer drag.

Cancellation also ends the visible selection preview and restores the pre-pointer node/edge
selection. Pan updates require a live gesture; viewport synchronization is not a gesture.
The cancelled pointer release cannot clear the restored selection, and the next press starts
normally. Shift-clicking the pane preserves selection. Port anchors always isolate node/pan
gestures, even when their connection handle is disabled. Selection, node, and connection drags
do not implicitly auto-pan.

`useCanvasViewport` connects the controlled React Flow viewport to the existing viewport
session. Navigation commands, sidebar drops, and view-state persistence use that same
coordinate system. React Flow does not own draft history, compile/save/execute, or Dockview layout.

`CanvasOverlays` receives a discriminated `graph` / `palette` / `execution` model from
the controller and must not assemble application commands.

### Other consumers

Use the narrow capability matching the caller:

```tsx
const { constants, loaded, saving } = useGraphConstants(graphPath);
```

Project Explorer obtains its active resource through
`features/application/sidebar/useActiveProjectGraph.ts`.

## Interface rules

| Capability / Hook                  | Interface                                                    | Mount/caller            |
| ---------------------------------- | ------------------------------------------------------------ | ----------------------- |
| `WorkbenchCommandCapability`       | Menu, keyboard, and welcome actions composed by the app      | `WorkbenchWindow` props |
| `useGraphConstants(graphPath)`     | Constants in the active graph draft                          | Event/Function Details  |
| `useEditorCanvas({ mode, scope })` | Panel-scoped Canvas `commands` / `workspace` / `interaction` | `GraphCanvasController` |

Do not rebuild a broad editor/group aggregate, spread unrelated values through
a subtree, or mirror Dockview topology in a store. Add or reuse a named slice
at the caller seam instead.

Repository-wide dependency direction and Dockview authority rules are defined in [`.rules`](../../../../.rules).

## Related modules

- Graph CRUD UI: `features/application/dataManagement/useGraphManagement.ts`
- Canvas cancellation, mutation contracts, and navigation bounds: `features/core/canvas/`
