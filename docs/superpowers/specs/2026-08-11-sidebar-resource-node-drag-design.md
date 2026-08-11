# Sidebar Resource and Node Drag Design

## Goal

Make the relevant leaf items in the Graphs, Nodes, Variables, and Data sidebar tabs draggable into an editor graph while preserving the existing drop semantics. Draggable sidebar rows use the same pointer cursor as collapsible sidebar rows instead of grab/grabbing cursors.

## Scope

The change covers:

- Function items in the Graphs tab.
- Catalog node items in the Nodes tab.
- Local and global variable items in the Variables tab.
- Database items in the Data tab.
- Shared cursor styling for draggable sidebar rows.

Event graph dragging already shares the graph-resource path and remains supported. Worksheet and command rows are outside this change.

## Existing Semantics to Preserve

Function dragging keeps its current behavior:

- A normal Function drag uses the graph-resource payload and opens the function graph in the target editor group or tab bar.
- Holding Shift while dropping a Function into a graph resolves the current backend-issued function descriptor and creates a Call node.

Other supported resource drops continue to create nodes from backend-issued descriptors:

- Node Catalog item: its declared creation descriptor.
- Local or global variable: the current variable Get descriptor.
- Database: the current dataframe source descriptor.

No frontend code synthesizes resource identities, revisions, ports, or creation arguments.

## Architecture

All drag sources continue to use the existing `dnd-kit` context and the centralized contracts under `src/features/core/dnd/`.

The Workspace remains responsible for drag lifecycle and routing. Sidebar tabs only provide typed payloads and visual rows. Graph drop handlers continue to resolve and execute node creation.

No native HTML drag-and-drop path or second drag framework is introduced.

## Components

### Catalog node items

`SidebarNodesTab` will render each localized Catalog item through the shared draggable sidebar row shell rather than a plain `div`.

Each row supplies a `node-template` payload containing:

```ts
{
  type: DRAG_TYPES.NODE_TEMPLATE,
  template: {
    title: item.title,
    descriptor: item.creation,
  },
}
```

The descriptor is forwarded unchanged from the authoritative localized Catalog response. The existing title and node type ID remain visible in the row.

### Function items

`SidebarGraphRow` already supplies a graph-resource payload through `buildSidebarDragData`. This path remains unchanged so ordinary and Shift-modified drops retain their current meanings.

### Variable and database items

`SidebarVariableRow` and `SidebarDataRow` already look up exact resource-bound descriptors in the current Catalog and produce node-template payloads. These paths remain unchanged.

If the current descriptor or resource path is unavailable, the row must not construct a guessed payload. Its existing Catalog/index refresh and warning-toast behavior remains in place.

### Cursor behavior

`SidebarDraggableItem` will use `cursor-pointer` whenever a payload is draggable. It will remove both `cursor-grab` and `active:cursor-grabbing`.

This matches `sidebarGroupRowClass`, which is used for collapsible sidebar rows. Drag overlays and editor tab cursors are outside this change.

## Data Flow

1. The sidebar receives a graph resource or authoritative Catalog item.
2. The row supplies a centralized `SidebarDragPayload` to `useDraggable`.
3. Workspace parses the payload through the existing DnD guards and stores drag preview state.
4. The existing editor drop router determines the target and modifier-key behavior.
5. Node-template drops create a node from the supplied descriptor. Graph-resource drops preserve existing graph-open or Shift-to-call behavior.
6. Invalid targets finish the drag without creating a node.

## Error Handling

- Missing resource descriptors leave variable/database dragging disabled and trigger the existing refresh path when the user attempts interaction.
- Invalid or unsupported payloads remain rejected by the centralized DnD guards.
- Drop failures continue to use the existing application toast/error handling.
- No fallback descriptor or resource path is synthesized in the view layer.

## Testing

Focused regression coverage will verify:

- A Catalog node row registers a node-template payload containing the exact `item.creation` descriptor.
- Nodes remain rendered with their title and node type ID.
- Function items retain graph-resource payloads and existing drop semantics.
- Local variable, global variable, and database rows retain their exact current resource descriptors.
- Draggable sidebar rows use `cursor-pointer` and no longer expose grab/grabbing classes.
- Descriptor-unavailable variable/database rows remain disabled and retain refresh behavior.

Existing canvas-drop tests remain the authority for ordinary Function drops, Shift-Function Call creation, and node spawning from templates.

## Non-goals

- Changing Function drag/drop semantics.
- Making worksheets or command history rows draggable.
- Adding native HTML drag support.
- Synthesizing node descriptors in React.
- Changing editor-tab or drag-overlay cursors.
