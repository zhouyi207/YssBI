# Document editors

> Status: Current
> Scope: Mind and Markdown editors, external reference previews, transient input buffers and resource integration
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
the content without reserving space, leaves room for the native scrollbar, and switches
between editing and a rendered preview using the shared `MarkdownRenderer`, which supports GitHub Flavored Markdown
(including tables) and KaTeX math. Preview reads the current input buffer; the
textarea stays mounted while hidden to retain editing state.
Single tildes stay literal for regression model notation such as `y ~ x1 + x2`;
strikethrough uses double tildes (`~~text~~`) in the shared Markdown pipeline.
Parser options live in `src/shared/ui/markdownRendering.tsx` and are shared with
Assistant output. Context-sensitive statistical writing rules belong to the
[report-writing skill](../../../src-tauri/crates/yss-harness-core/skills/statistical-report-writing/SKILL.md),
which the Harness Host loads into Assistant requests. The renderer does not guess
or rewrite currency, significance markers, programming identifiers, or table pipes.
Single-dollar `$...$` formulas stay inline at a size close to surrounding text;
paragraphs, lists and table cells containing math use a comfortable line height.
Double-dollar `$$...$$` formulas display on their own centered line, whether their
delimiters are written on the same line or separate lines. Formula layout is
selected from parsed math nodes, leaving code examples and escaped dollars literal.
The host and Julia UI pin KaTeX to a version supported by both `rehype-katex` and
`micromark-extension-math`, so all consumers resolve consistently without dependency
overrides. Formula markup and CSS come from the same package, including fraction
and subscript height calculations; direct imports use its bundled TypeScript
declarations. Keep this version alignment when updating math dependencies.
Preview uses Tailwind Typography's type scale in a centered column that fills the
editor pane's available width, with responsive side padding instead of a fixed
character-width cap. Node documentation uses the same typography with the compact
scale. Shared prose colors and code-block
surfaces follow application theme tokens; Shiki token colors switch with the app's
light/dark mode, including OLED. The highlighter is loaded lazily and shared across
views, with explicit language imports for common web, scripting and data languages.
Code remains readable before loading, and unsupported languages remain plain text.
Inline code keeps Typography's code styling without decorative backticks;
literal backticks inside code remain part of the content.

Shared `MarkdownLink` prevents native click and middle-click navigation and delegates
HTTP(S) links through `MarkdownLinkContext`. Workbench composition supplies
`openReferenceLink`, which opens/reuses a right-side reference tab without replacing
the application WebView; the node-help modal closes after the panel opens successfully.
Outside the workbench, links use the existing system-browser opener. Assistant retains
its existing external-browser link handler. Unsupported schemes are rendered as text.

`ReferencePanel` owns only document presentation: a URL/title, reload and browser-open
controls, and an independent frame. Web pages are sandboxed without top-navigation
permission. Remote `.pdf` URLs are fetched using CORS without credentials, bounded
by `MAX_REFERENCE_PDF_BYTES` in `services/platform/referencePdf.ts`, and checked for
a PDF header before receiving a local Blob URL. The native PDF viewer displays that
Blob without an HTML sandbox, which would block native PDF plugins. This supports
PDF hosts such as the World Bank that allow CORS while denying direct frame embedding.
Reload/unmount aborts pending reads and revokes Blob URLs. Query strings and PDF page
fragments remain intact. The browser-open action stays available for hosts that disallow
CORS/embedding, downloads beyond the preview budget or platforms without native PDF
support; iframe load events cannot reliably diagnose display failures and are not
reported as successful previews.
Reference tabs use the existing workbench float/close/drag actions and are not restored
when the window reopens. They do not create project resources or document drafts.
Wide tables, code blocks and display formulas scroll within the reading column.
Tables size to their content and stay centered within the available column width.
Table scroll containers have no decorative frame; spacing belongs outside the
container, with the table's vertical margins reset so no blank bands appear inside it.
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
Initial loading belongs to the editor view. Split panes share an in-flight read for
the same project and file; a view with a matching resource revision reuses its snapshot.
Opening an existing tab and installing an edit response do not trigger another read.

External removal keeps an editor's unfinished text available instead of closing it.
An explicit delete receipt authorizes removal. Explicitly discarding a missing file
releases its retained inputs and any Rust resident edits. Rename moves retained Mind
and Doc input buffers, including text entered while the command was in flight.

`mindActions` / `MindService` and `docActions` / `DocService` call their own IPC commands.
`createFileActions` shares queuing, version checks and publication; it does not combine
Markdown and tree edits. Current synchronization uses command snapshots and the existing Project resource
publication notifications. There is no additional document channel, frontend
document authority, automatic save or independent renderer persistence format.
Native undo/redo within a text input is available; document-level Mind history is not
part of the current editor. Harness reads and edits these resources through the same
typed Application owners: Mind batches preserve tree constraints and Doc range edits
preserve text outside the requested Unicode character range. Resource publication
updates open editors; version checks retain conflicts with unfinished UI inputs.

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
   With long documents in both modes, verify a visible gap between the button and the
   native scrollbar, including in narrow split panes, and that the scrollbar remains draggable.
   Verify Markdown tables render with their header, cells and column alignment,
   including inline math in cells and display math outside tables.
   Check `$x^2$` and `$\frac{a}{b}$` within prose and table cells: formulas stay inline,
   with enough line height for fractions and subscripts. Compare same-line
   `$$x^2$$` with multiline double-dollar formulas: both are centered independently
   of surrounding text. Dollar delimiters inside code examples or escaped in prose
   must remain literal.
   Check heading spacing, ordered/nested lists, links, quotes and code blocks in both
   the reading preview and compact node documentation. Switch Light Modern, Dark
   Modern and OLED while viewing highlighted SQL, Python, Rust and JavaScript;
   verify colors update without changing content. Check unknown/unlabelled code
   fences, narrow split panes and horizontal scrolling of wide tables and formulas.
   Resize the window and split panes; verify the centered preview expands and shrinks
   with its pane. Check tables alone and between paragraphs: no enclosing frame or
   blank bands above/below their content, while normal paragraph spacing remains.
   Verify short tables fit their content and remain centered; wide tables shrink
   or scroll within narrow panes without clipping their first or last column.
   Use Ctrl+S before blurring and Save from the File menu in both modes. Verify saved
   text, tab dirty indicators and Save/Discard/Cancel when closing the last tab or project.
4. Edit a file externally while its buffer is dirty. Verify a stale edit/save is
   rejected, input remains available and Discard in the close confirmation reloads the saved file.
5. Open split panes, switch projects during a pending read, and rename an open file.
   Verify late results cannot restore an old path or overwrite the new project.
6. Create, read, edit, save and open Mind/Doc through Assistant; verify the project tree,
   editor contents and Details refresh from committed resource events. Repeat with a
   pending UI text buffer and verify a stale edit cannot silently overwrite it.
7. Open the Theil World Bank PDF link from Details and the node-help modal. Verify the
   workbench remains present, the modal closes after opening, and the reference tab can
   close, float, resize and dock. Reopen the same link to verify reuse. Test reload,
   the system-browser action, and a website that rejects frames. Reset returns the
   reference to the right side; restarting the window does not reload external pages.
