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
Connection feedback and its preview consume versioned Rust connection candidates; palette position comes
from the owned context menu. No Pin projection or second coordinate snapshot is kept in the
interaction store.

Canvas mutation handlers return only applied/failed status. Failure reporting consumes graph
identity and action; it does not construct or carry unused localized message payloads.

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

`features/core/editor/detail/editorDetailPolicy.ts` owns the two resource-tab Details
policies. Event Graph, Function Graph and Mind follow selection: one node shows node
details, while no selection or multiple nodes show file information. Doc, Chart and
Data always show the current resource. Tab activation reads the owning pane's selection
through that same resolver; it never carries another pane's node into the newly active tab.

The native file/database opening flows select the resource panel. Application
then calls `revealActiveEditorDetails` to synchronize the resource target and reveal Details
view for every resource kind. Sidebar rows and their Open actions use that same path,
without kind-specific inspection branches. Passive layout synchronization and pane
movement keep their existing visibility behavior; they only update Details content.
Synchronization checks the captured panel's group and resource identity against the
native active editor. It does not enqueue another activation or keep an application
activation counter. A late open result cannot reselect a tab the user has already left.

Opening Chart, Mind and Doc only opens the panel; their views own initial loading.
Chart views share `loadChartDocumentForView` requests and reuse the existing document
draft. A response is installed only for its captured project, resource revision and
read lifecycle; closing the last panel invalidates unfinished reads. Editor and Details
share the loading/retry hook. Mind and Doc similarly share pending reads and skip
refreshes when their snapshot already matches the resource revision.

`setInspectionContext` accepts an explicit resource/pane scope and only publishes for
the native active editor. Node selection and clearing selection use this entry, including
Escape. `setDetailContext` deduplicates equivalent targets. Close and rename share the
policy module's resource identity helpers; publication and hydration retain Details by
the same resource ownership rules used by editors. Explicit log, catalog and problem
inspection keep their own existing targets.

Details owns node parameters, configuration, ports, diagnostics and documentation;
explicit node-details commands reveal that same fixed panel. A missing resource and a
chart that is loading or failed retain file identity and show a status instead of the
no-selection state. Chart loading can be retried. History availability subscribes to the
active Graph. Node creation validates its captured target when invoked, without subscribing
every mounted canvas to global tab selection; an unavailable canvas target cannot fall back
to another panel.

`openReferenceLink` normalizes remote HTTP(S) documentation URLs and uses the workbench's
`openReference` capability. References open beside Details, reuse their complete URL,
and retain the current graph/document. The shared Markdown renderer receives this action
from Workbench composition; it does not own layout or navigate the main WebView.

Node Details displays each parameter as a top-level section, expanded by default. Parameter
groups retain their declared ordering and descriptions without an outer Parameters heading;
the section title is not repeated inside its control. Collapsing one parameter affects only
that section, and diagnostic focus keeps the graph/node/parameter identity on its wrapper.
Every parameter uses one tagged `editor` object: for example `{ "kind": "number" }`,
`{ "kind": "select", "options": ["OLS", "WLS"] }`, or a `projectColumns` / `filterPredicate`
descriptor carrying its schema-derived choices and editable value. The renderer selects the
existing control using `editor.kind`; grouping, display metadata and presentation remain separate.
The panel and parameter sections use memoized boundaries over the existing structurally
shared projection. Each section subscribes only to its own parameter diagnostics; unchanged
parameters do not rerender with another parameter edit. Column rows reserve space for order
controls and keep a fixed height. Pending column edits use `aria-busy`, `aria-disabled` and
guarded handlers; they block duplicate input without removing focus or fading the list.
Native disabled controls remain for actual selection/reordering constraints.

Graph synchronization uses the existing cursor-bound JSON deltas, with Immer for atomic
installation and Zod for envelope/operation validation. Array splices preserve unchanged
entries; the Zustand projection installer matches parameter groups and parameters by key,
so inserting or removing a conditional field does not replace its neighbors. Protocol,
bounds and read-only recovery belong to [Graph projection synchronization](../../../../src-tauri/crates/yss-application/src/ipc/README.md#graph-projection-synchronization).

Node parameter editors commit changes through `setNodeParameters` and the existing Graph
mutation FIFO, without an Apply button or implicit file save. Toggles, selections, column
ordering and domain row actions submit immediately. Text and numeric input submit on Enter
or when editing finishes; composite filter/domain editors wait until focus leaves their
controls so a selection or row action can submit the complete change atomically. Escape
restores the projected value. Unfinished or duplicate domain entries remain local input;
Rust validates submitted parameters and owns undo/redo. Rejected edits retain the published
projection and show the existing field error. Required column selections retain at least
one column, following the Rust-issued `allowEmpty` flag.

Manual acceptance for this interaction covers column selection/reordering and independent
labels across multiple selectors, filter column/operator/literal changes (including empty
text and exact large integers), domain code/label edits and row actions, and failure,
undo/redo and node-switch behavior. UI unit tests are not added for this flow.

`useDetailPanelModel` reads the existing editor collections and produces one discriminated
model for rendering; the chart branch includes its path, name and document. There is no
separate Details target-resolution or resource-projection facade.
When a focused graph node disappears from the published projection, the model uses
that graph's resource identity to display Event/Function Details instead of a missing-node message.

Mind activation publishes its file path, owning panel ID and resolved topic ID to that
same Details context. Its topic selection and collapsed branches use the existing pane
state; the Details form consumes the policy's target and uses the shared detail controls.
Both canvases use the same basic shortcut resolver and viewport fitting rules.
Mind handles its shortcuts within the owning canvas, using the existing DOM target
and input/modal guards; Graph retains the workbench command dispatcher.
Rename remaps the context and closing its owning pane clears it. Topic input
retention and commands are owned by [File operations](../resource/README.md).

Activating a cached graph reuses its ready loading status and loaded document state instead of publishing duplicate updates. Graph panels subscribe to their own loading status; UI intent delivery subscribes only to project identity, without a broader project-state aggregate.

Focus synchronization is synchronous and never loads, retries or unloads graphs. Canvas gestures call the same focus coordinator directly. Visible panels and explicit data-dependent use cases call the same `ProjectIOStore.loadGraph` entry, which deduplicates in-flight loads and reuses cached graphs. Project restoration first ensures visible graphs, then synchronizes the active editor's focus. Cache cleanup follows successful loads and panel closure instead of every focus switch; the old activation/suspension queue and bootstrap retries are removed.

Menus capture the native active editor across the main layout and floating tabsets. Workbench keeps one native active tabset across those layouts; no separate active-editor store is introduced. Explicit canvas actions capture their own visible panel. Both revalidate identity before committing; the menu target also revalidates the active selection. Keyboard node commands resolve the owning panel from the DOM event path or focused element, while inputs, menus, modals and import progress retain their shortcuts. Execution controls use the canvas graph path and stay mounted while focus changes.

Use the narrow capability matching the caller:

```tsx
const { constants, loaded, saving } = useGraphConstants(graphPath);
```

Event/Function Details displays each graph constant in one row. Only its leading handle
starts a dnd-kit drag, leaving the name, type and value controls editable. The shared
workbench drag flow carries the graph path and constant ID, displays the constant name,
activates the target canvas and uses that pane's viewport to convert the release point.
Dropping on another graph, outside a canvas, after project replacement, or with an
unavailable target or constant does not create a node. Valid drops submit the existing
Rust constant-node mutation through the Graph edit FIFO; values remain owned by the
graph constant. Saving gates the mutation, and Escape cancels the drag and its overlay.

Project Explorer obtains its active resource through
`features/application/sidebar/useActiveProjectResource.ts`.

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

- File resource operations: `features/application/resource/fileManagement.ts` and `resourceActions.ts`
- Canvas cancellation, mutation contracts, and navigation bounds: `features/core/canvas/`

`collectEditorFiles` owns open-file enumeration and deduplication for queue barriers and dirty-file collection. `collectDirtyEditorPanels` filters that current list for close prompts and save-all. Window/project close, panel close and save-all await the file handlers through `settleEditorFileEdits` before collecting dirty state. `saveAllDirtyDocuments` and panel-close saves dispatch through the typed file-resource registry. Closing retains project identity checks and captured discard versions. Publication, panel pruning and file projection cleanup share `shouldRetainResourceEditor`; explicit deletion receipts authorize removal separately. Mind and Markdown use Rust-owned document snapshots; see [Document editors](../../../modules/document-editor/README.md).
