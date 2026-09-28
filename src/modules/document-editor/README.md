# Document editors

> Status: Current
> Scope: Mind and Markdown editors, transient input buffers and resource integration
> Canonical owners: This module owns document canvases; Details owns property forms; Application resource actions own commands; Rust Project owns current documents
> Update when: Editor interaction, resource operations or projection ownership changes

`MindFileEditor` and `DocFileEditor` are registered separately by the workbench composition
root. Their independent components consume `MindSnapshot` and `DocSnapshot`; the generic
`FileEditor` shell provides loading, keyboard saving and error presentation.
The editors have no file-action toolbar, save-status row or in-panel discard confirmation.
The Project sidebar and File menu create these resources through the typed
`fileResourceHandlers` registry in `features/application/resource/resourceActions.ts`.
That registry covers creation, copying, rename, delete and explicit save for Event,
Function, Chart, Mind and Doc. Database operations keep their existing owner.

Mind fills its content area with the React Flow node canvas. Canvas navigation uses
gestures, without floating zoom/fit controls or the React Flow attribution badge.
Mind and the node graph share `flowCanvasInteractionProps`, canvas shortcut mapping
and viewport limits/fitting. Left drag selects intersecting nodes; Shift/Ctrl/Cmd
adds or toggles selection. Middle/right drag or Alt+drag pans; wheel/pinch zooms.
Only the active selection gesture shows a rectangle. After release, selected nodes
are indicated by their own highlights. Ending or cancelling selection clears the
library's rectangle and group-overlay state without clearing the selected node IDs.
Plain node clicks reveal Details, while modified clicks only update selection.
New Mind panes start without a selected topic. The common editor Details policy
resolves the selected topic or file-level fallback on activation and selection changes.
Mind disables individual and group node dragging. Escape cancels a pan or selection
rectangle and restores its starting state; without
an active gesture it clears selection. A plain empty-space click clears selection
and returns Details to the current Mind file's information, as in the node graph.
Shift-clicking empty space retains selection.
Ctrl/Cmd+A selects the currently expanded nodes, F fits the selection, Home fits the
visible tree, and Delete/Backspace removes selected branches while protecting the
root. A selected ancestor covers its descendants in the deletion batch. Input
fields and modal dialogs keep their native shortcuts. Hiding a pane cancels its
unfinished gestures.
The workbench Details panel
owns the selected topic's text, parent, add/delete, ordering and collapse
controls, reusing `DetailPanelShell`, `DetailForm`, `DetailFieldRow`, `DetailTextarea`
and the existing Select and Button controls. `DetailTextarea` is also used by node
parameter details. Only a single selected topic has editable details; empty or
multiple selections use `FileDetailPanel`, which also serves Doc and resource
loading/error states and reuses `DetailForm` and `DetailReadonlyField`. Mind has no separate
properties sidebar inside its editor.
`mindProjection.ts` derives edges and a deterministic horizontal tree layout from
parent links, sibling order and collapsed branches. Depth determines each column;
subtree spans determine vertical spacing and center parents over their branches.
Structure or collapse changes recompute all positions. Coordinates are rendering
results and are neither editable nor persisted in Mind documents.
Adding topics, editing text, reparenting, ordering siblings and deleting a branch
use Rust typed edits. Selection and collapsed node IDs use the existing
panel-scoped `EditorPaneState`. Details captures
the owning panel ID, so split panes keep independent selections and collapse state.
The domain can retain resource references; a reference picker and Mermaid import/export
are not exposed by this editor.

Doc starts with a full-width Markdown textarea. A floating top-right button overlays
the content without reserving space and switches between editing and a rendered preview
using the shared `MarkdownRenderer`. Preview reads the
current input buffer; the textarea stays mounted while hidden to retain editing state.
Saving is available through Ctrl+S / Cmd+S and the File menu in both modes.
Doc's Details always shows its file information, including while switching between
editing and preview. Tab activation, close and rename follow the common resource policy.
Input buffers only represent ongoing text composition. They capture their base
editing version, flush on blur or explicit Save, and never silently overwrite a
newer backend version. Project save-all flushes through the same resource handler.
Mind topic buffers stay registered while Details changes its target, including
failed submissions, and are reused when returning to the topic. Closing the final
file panel, resource removal and project replacement release these buffers.
The generic close workflow waits for already submitted edits and supports Save,
Discard and Cancel. Discard remains available when a text buffer cannot be submitted.

`mindProjectionStore` and `docProjectionStore` independently retain typed read snapshots
using the generic `fileProjectionStore` mechanics. Pending operations live in the
application edit queues, which all close/save-all entry points await.
Reads carry invalidatable ownership, so close, rename, project replacement or newer
results prevent late reads from restoring stale projections. Resource publication
remaps editor paths and refreshes changed open documents. The editing queue is keyed
by project and file; split panels observe the same Rust document. Concurrent text
buffers keep their captured version and report conflicts rather than merging them.

External removal keeps an editor's unfinished text available instead of closing it.
An explicit delete receipt authorizes removal. Explicitly discarding a missing file
releases its retained inputs and any Rust resident edits. Rename moves retained Mind
and Doc input buffers, including text entered while the command was in flight.

`mindActions` / `MindService` and `docActions` / `DocService` call their own IPC commands.
`createFileActions` shares queuing, version checks and publication; it does not combine
Markdown and tree edits. Current synchronization uses command snapshots and the existing Project resource
publication notifications. There is no additional document channel, frontend
document authority, automatic save or independent renderer persistence format.
Native undo/redo within a text input is available; document-level Mind history and
Harness tool registration are not part of the current editor.

Manual acceptance on the desktop:

1. Create Mind and Doc files from both the File menu and sidebar; verify tab reuse,
   rename, duplicate, delete and reopening the project.
2. In Mind, select topics and use the existing Details panel to add sibling/child
   topics, edit labels, reparent, reorder and collapse branches. Verify the
   canvas fills its editor without file controls or save status, split panes retain
   independent selection/collapse state,
   switching files retains unfinished text, and renaming/closing updates Details.
   Click empty space to clear node highlights and show the Mind file name in Details;
   verify Shift-click keeps the selection and selecting a topic restores its fields.
   Verify that individual and selected groups of nodes cannot be dragged, and that
   structural edits and collapse/expand automatically rearrange the tree. Save with
   Ctrl+S from Details and reopen; verify text, structure and the resulting layout.
   Compare left-drag selection, modifier multi-selection, wheel/pinch
   zoom and middle/right/Alt panning with the node graph. Check Ctrl/Cmd+A, F, Home,
   Delete/Backspace and Escape, including Escape before releasing a pan or selection
   rectangle and switching panes mid-gesture. Text fields retain native editing keys.
3. In Doc, verify the floating top-right button reserves no content space, remains
   available while scrolling and switches between full-width editing and preview,
   renders current input and preserves text and native undo when returning to editing.
   Use Ctrl+S before blurring and Save from the File menu in both modes. Verify saved
   text, tab dirty indicators and Save/Discard/Cancel when closing the last tab or project.
4. Edit a file externally while its buffer is dirty. Verify a stale edit/save is
   rejected, input remains available and Discard in the close confirmation reloads the saved file.
5. Open split panes, switch projects during a pending read, and rename an open file.
   Verify late results cannot restore an old path or overwrite the new project.
