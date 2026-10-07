# Project file operations

> Status: Current
> Scope: File kinds, lifecycle handlers, UI orchestration and resource identity
> Canonical owners: resourceActions.ts owns the file handler registry; Project owns persistence and authoritative state
> Update when: File creation, lifecycle dispatch, identities or public file commands change

Event Graph, Function Graph, Chart, Mind and Doc are independent file kinds. `fileResourceHandlers`
registers their creation, rename, duplication, removal, save and sidebar category.
Callers select `fileResourceHandlers[kind]`; Event Graph and Function Graph have individual
registrations and creation rules. Shared helpers perform transaction submission
and ownership checks. Database rename/removal retains its database mutation owner.

`createFile`, `duplicateFile` and `deleteFileWithConfirm` in `fileManagement.ts`
own UI workflows; `openFileInEditor` selects the file's editor adapter. The File
menu, sidebar and keyboard call the same operations. A newly created file opens
its editor and expands its category. Generic workbench commands use
`saveActiveFile`, `saveProjectAs` and `openProject` to describe their actual scope.

The Project sidebar owns rename input and errors through `SidebarRenameDialog`.
`application/sidebar/useProjectActivityActions` submits those values through `renameResource`.
It captures project identity when opening a rename form and revalidates it on submission.
Database deletion similarly checks the original project after confirmation; a replaced project
cannot receive the old dialog's command. File deletion retains the same check in `deleteFileWithConfirm`.
The existing file/database command owners still capture and validate resource revisions when submitting.

`EventGraphService` and `FunctionGraphService` bind separate create/rename/duplicate/
remove IPC commands. Rust rejects a path belonging to the other file kind before
calling shared node-document persistence. Event Graph starts with an empty node document;
Function Graph initializes its signature and entry/return nodes. Both reuse the node-graph
editor, semantic analysis, execution and revision machinery. `GraphDocument`
describes that shared content structure, not a project file kind.

Project index consumers read `eventGraphs` and `functionGraphs` independently. Function
entries always include their signature, signature revision and editor projection.
ResourceStore publishes these function metadata candidates with resource names and revisions
in the same project snapshot update, and clears them together on project reset.
`nodeFileEntries` is a derived view for shared node-editor consumers. The sidebar
and project inspection expose each concrete file kind directly.

Graph sessions, normalized entities and result summaries share ResourceStore with their resource
metadata and document flags. Graph receipts are prepared and validated before one publication;
Save uses the Rust resource revision and clears the graph's save lock in that same update.
Project snapshots install their graph candidates through `setSnapshot`, preserving retained dirty
sessions and removing discarded sessions together with their document state. Rust remains the
owner of Graph documents and history; result payloads and report leases remain with Results.

Mind and Doc likewise have separate `mindActions` / `docActions`, services, typed
snapshots and IPC endpoints. ResourceStore owns both snapshot collections together
with their document flags and resource summaries. Their index entries live in `minds` / `docs`; mutation
keys are `mind` / `doc`. Mind commands accept only tree edits and Doc commands only
Markdown edits. `createFileActions` and `createFileContentService` share transport,
queueing and publication mechanics without a mixed content or editing contract.

Accepted Mind/Doc snapshots publish their content, loaded, dirty and missing flags
together with the resource revision through Core's ResourceStore. Rename installs
the destination and removes the source in the same update; release removes content
and document flags together. Dirty
state includes retained, unsubmitted inputs; a loaded snapshot cannot restore an
externally missing resource's index membership. The resource summary and document
flags are committed in one Immer update, preserving unchanged records. Equal snapshots
reuse their branches and do not notify subscribers. Read ownership also captures the
resource revision and existence, rejecting a late first read after index removal.

Command acceptance correlates the parsed snapshot and resource delta with the captured project,
operation, file kind, path and editing version before installing content or changing buffers.
Creation and duplication require a matching creation delta; rename takes its destination from the
matching move receipt. Source revisions and retained editing sessions must match the command,
and snapshot revisions must match the returned delta. A null snapshot authorizes removal only for
Delete or Discard with a corresponding removal delta. This check shares the existing command queue
and publication coordinator; structural parsing remains at the Service boundary.

`useFileTextInput` binds views to the versioned composition buffer in `fileTextInput`.
Every field supplies a stable input ID. `documentInputs` owns one buffer table keyed
by project, path and session/field identity; there is no separate transient registration
table. Save, discard, rename and release all traverse that same table.
Each retained buffer owns a Zustand store for text, dirty state, edit generation and
base version. Snapshot reads and subscriptions use that store directly; there is no
parallel mutable text snapshot or custom listener collection. Edit completion reads
the current generation before clearing dirty state, preserving newer input.
Text changes and edit completion recheck the same buffer's project lifecycle and release
state after notifying subscribers, before updating the resource's dirty flag.
Concurrent flushes recheck the same in-flight edit after every wait; only one caller submits
the next dirty generation using the preceding reply's version. A failed edit leaves the input
pending and rejects its waiters without an implicit retry.
`documentInputs` retains Mind topic buffers across Details target changes so Save,
Discard and close still reach unfinished text after the form unmounts. These are
transient inputs, not a second document authority; the last file close, resource
explicit removal and project reset release them. Doc Markdown buffers use the same
retention mechanism. Committed renames move registrations to the new path; only an
exact matching base version advances across that rename, preserving conflicts for
stale text. Both command replies and earlier publication events use this path.
Rename transfers the captured registrations before notifying their buffers and only
remaps inputs still owned by that target registration. Notifications may release or
replace registrations without the old rename overwriting them. A project lifecycle
change aborts the old rename continuation before its caller publishes file state.
External file disappearance retains dirty snapshots and buffers until explicit
discard; editing state cannot restore a missing file's index membership.
`applyMindEdits` flushes these inputs and
rechecks project/document ownership before applying structural changes.
`deleteMindNodes` protects the root and reduces a multi-selection to disjoint
branches before sending a single edit batch through that same input/ownership barrier.

The resource wire kinds are `event_graph` and `function_graph`; Rust variants are
`EventGraph` and `FunctionGraph`. IPC creation uses `create_event_graph` /
`create_function_graph`, with corresponding rename/duplicate/remove commands.
Project index fields are `eventGraphs` / `functionGraphs`. Persisted paths retain
`events/<name>.yssbi-event` and `functions/<name>.yssbi-function`; graph file kind
fields use the current wire kinds. Function-signature deltas retain their separate
`function` protocol tag. Runtime and IPC events retain their event terminology.

Frontend resource keys use `toResourceUri(kind, id)` for every resource kind. The
ID is opaque; UI code does not infer kind from its path. `buildFileResourceMeta`
creates common metadata. Editing versions, resource revisions and panel identity
keep their existing owners and are not collapsed into this key.

File dispatch and publication tests are in `resourceActions.test.ts` and
`fileManagement.test.ts`. Editor interactions use desktop manual acceptance;
see [Document editors](../../../modules/document-editor/README.md).

Each file handler exposes an edit-queue barrier. Window/project close, panel close
and save-all wait through `settleEditorFileEdits` before reading dirty state. Queue
ownership is the only pending-operation state; snapshots carry authoritative file
versions without a second UI revision counter.
Rename, duplicate and Save capture the original project lifecycle before flushing inputs
and recheck it before entering the command queue. Discard rechecks after the queue and
after clearing captured input buffers, before updating dirty flags or releasing content;
synchronous input notifications cannot redirect that continuation to a successor project.
Save also verifies the original lifecycle before reporting its final clean state.
