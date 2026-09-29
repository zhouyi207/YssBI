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
`useProjectActivityActions` submits those values through `renameResource`.

`EventGraphService` and `FunctionGraphService` bind separate create/rename/duplicate/
remove IPC commands. Rust rejects a path belonging to the other file kind before
calling shared node-document persistence. Event Graph starts with an empty node document;
Function Graph initializes its signature and entry/return nodes. Both reuse the node-graph
editor, semantic analysis, execution and revision machinery. `GraphDocument`
describes that shared content structure, not a project file kind.

Project index consumers read `eventGraphs` and `functionGraphs` independently. Function
entries always include their signature, signature revision and editor projection.
`nodeFileEntries` is a derived view for shared node-editor consumers. The sidebar
and project inspection expose each concrete file kind directly.

Mind and Doc likewise have separate `mindActions` / `docActions`, services, snapshot
stores and IPC endpoints. Their index entries live in `minds` / `docs`; mutation
keys are `mind` / `doc`. Mind commands accept only tree edits and Doc commands only
Markdown edits. `createFileActions` and `createFileContentService` share transport,
queueing and publication mechanics without a mixed content or editing contract.

`useFileTextInput` binds views to the versioned composition buffer in `fileTextInput`.
Every field supplies a stable input ID. `documentInputs` owns one buffer table keyed
by project, path and session/field identity; there is no separate transient registration
table. Save, discard, rename and release all traverse that same table.
`documentInputs` retains Mind topic buffers across Details target changes so Save,
Discard and close still reach unfinished text after the form unmounts. These are
transient inputs, not a second document authority; the last file close, resource
explicit removal and project reset release them. Doc Markdown buffers use the same
retention mechanism. Committed renames move registrations to the new path; only an
exact matching base version advances across that rename, preserving conflicts for
stale text. Both command replies and earlier publication events use this path.
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
