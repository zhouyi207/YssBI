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

Use `mode: 'preview'` for a hidden or saving FlexLayout panel. Visible split panes remain interactive when another pane or sidebar receives input focus. Preview disables
editing, selection, zoom/pan, and context-menu actions; its sidebar drop route remains
registered so the workbench can activate the target before dropping.

The graph-editor module mounts one React Flow provider per panel/group/resource.
Its nodes and edges are derived from the editor projection, not saved graph documents.
Node/port views remain custom React components; dynamic handle IDs use the existing port keys.
Controlled nodes retain measured dimensions and dragging flags, and unchanged nodes keep their
references. Measurements are accepted in preview mode as well; losing them would hide nodes
and reset connection anchors on every update.
`useCanvasInteraction` captures the visible panel and project identity independently of the active central tab, and grants a current, cancellable gesture; the renderer keeps its
position previews local and submits one existing mutation at gesture end. Escape,
hiding, save locking, graph close, and project replacement invalidate the gesture.
Late mutation completion only clears that mutation's preview, never a newer drag.

Pending node creation retains only the structured source port address and panel ownership.
Connection feedback and its preview read the current graph projection; palette position comes
from the owned context menu. No Pin projection or second coordinate snapshot is kept in the
interaction store.

Cancellation also ends the visible selection preview and restores the pre-pointer node/edge
selection. Pan updates require a live gesture; viewport synchronization is not a gesture.
The cancelled pointer release cannot clear the restored selection, and the next press starts
normally. Shift-clicking the pane preserves selection. Port anchors always isolate node/pan
gestures, even when their connection handle is disabled. Selection, node, and connection drags
do not implicitly auto-pan.

`useCanvasViewport` connects the controlled React Flow viewport to the existing viewport
session. Navigation commands, sidebar drops, and view-state persistence use that same
coordinate system. React Flow does not own draft history, save/execute, or FlexLayout layout.
React Flow handles wheel zoom and the canvas content transform. Core viewport helpers own
the shared session and grid alignment; they do not attach a second wheel listener or transform.

`CanvasOverlays` receives a discriminated `graph` / `palette` / `execution` model from
the controller and must not assemble application commands.

### Other consumers

`editorGroupContext` owns the shared native active Graph read. Problems, Output, Project sidebar and Assistant consume it; graph-session focus bookkeeping is not a UI selection authority. Session path remapping is committed by `projectPublicationSnapshot` with the authoritative resource snapshot.

Node selection synchronizes the existing Details context without opening another panel. Details owns node parameters, configuration, ports, diagnostics and documentation; explicit node-details commands reveal that same fixed panel. History availability subscribes to the active Graph. Node creation validates its captured target when invoked, without subscribing every mounted canvas to global tab selection; an unavailable canvas target cannot fall back to another panel.

Activating a cached graph reuses its ready loading status and loaded document state instead of publishing duplicate updates. Graph panels subscribe to their own loading status; UI intent delivery subscribes only to project identity, without a broader project-state aggregate.

Focus synchronization is synchronous and never loads, retries or unloads graphs. Canvas gestures call the same focus coordinator directly. Visible panels and explicit data-dependent use cases call the same `ProjectIOStore.loadGraph` entry, which deduplicates in-flight loads and reuses cached graphs. Project restoration first ensures visible graphs, then synchronizes the active editor's focus. Cache cleanup follows successful loads and panel closure instead of every focus switch; the old activation/suspension queue and bootstrap retries are removed.

Menus capture the native active editor across the main layout and floating tabsets. Workbench keeps one native active tabset across those layouts; no separate active-editor store is introduced. Explicit canvas actions capture their own visible panel. Both revalidate identity before committing; the menu target also revalidates the active selection. Keyboard node commands resolve the owning panel from the DOM event path or focused element, while inputs, menus, modals and import progress retain their shortcuts. Execution controls use the canvas graph path and stay mounted while focus changes.

Use the narrow capability matching the caller:

```tsx
const { constants, loaded, saving } = useGraphConstants(graphPath);
```

Project Explorer obtains its active resource through
`features/application/sidebar/useActiveProjectGraph.ts`.

## Interface rules

| Capability / Hook                  | Interface                                                    | Mount/caller            |
| ---------------------------------- | ------------------------------------------------------------ | ----------------------- |
| `WorkbenchCommandCapability`       | Menu and keyboard actions composed by the app                | `WorkbenchWindow` props |
| `useGraphConstants(graphPath)`     | Constants in the active graph draft                          | Event/Function Details  |
| `useEditorCanvas({ mode, scope })` | Panel-scoped Canvas `commands` / `workspace` / `interaction` | `GraphCanvasController` |

Do not rebuild a broad editor/group aggregate, spread unrelated values through
a subtree, or mirror FlexLayout topology in a store. Add or reuse a named slice
at the caller seam instead.

Repository-wide dependency direction and FlexLayout authority rules are defined in [`.rules`](../../../../.rules).

## Related modules

- Graph CRUD UI: `features/application/dataManagement/useGraphManagement.ts`
- Canvas cancellation, mutation contracts, and navigation bounds: `features/core/canvas/`
