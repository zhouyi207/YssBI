# Project model

> Status: Current
> Scope: ProjectData aggregate and authored Mind/Markdown document contracts
> Canonical owners: This crate owns in-memory models; yss-project owns editing, persistence and publication
> Update when: ProjectData or authored document shapes and invariants change

`ProjectData` contains typed graph, chart, database, `minds` and `docs` collections.
Event Graph and Function Graph have independent project index entries and application factories.
The resident `graphs` collection holds their shared node-graph editing contents;
it is not the project's file classification or navigation index.
`mind.rs` owns `MindDocument`, `MindEdit` and `MindPath`; `doc.rs` owns
`DocDocument`, `DocEdit` and `DocPath`. A Mind edit cannot carry Markdown changes,
and a Doc edit cannot carry tree operations. Their paths reject the other kind's
directory and extension. `file.rs` shares typed path mechanics, editing versions,
saved-content fingerprints and patch structure without combining their contents.
These models do not depend on a renderer, IPC or the filesystem. Existing graph
and chart bodies retain their dedicated model owners.

Mind files use `minds/<name>.yssbi-mind`, containing UTF-8 JSON with `rootId` and
`nodes`. Each node has `id`, `parentId` and `content`; optional `reference` identifies a project
file, graph node or database without copying its contents. References use existing
project paths/database identities; they do not introduce a second resource ID scheme.
References are preserved as authored, including references whose target no longer
exists; their existence is not a tree-validity constraint.

There is exactly one root, every other node has an existing parent, IDs are unique,
and every node reaches the root without a cycle. Array order determines sibling
order. Reparenting and subtree deletion are domain operations. Node coordinates are
derived entirely by the renderer's tree layout and have no document field or edit command.
Edges, layout results, selection, collapsed branches and renderer measurements are not
persisted. File names own resource display names; root content is the mind's topic.

Doc files use `docs/<name>.md` and contain plain UTF-8 Markdown, including normal
headings, lists, links and code fences. The typed IPC snapshot is a read contract
and is not wrapped around the saved Markdown file. No editor-specific AST,
renderer state, resource revision or editing session ID is written into either format.

Bounds are defined by `MAX_FILE_BYTES` in `file.rs` and `MAX_MIND_NODES` in `mind.rs`.
Mind validation uses an ID index and connected-node set; adapters may build their
own disposable child indexes. Validation happens before a candidate is committed.
See [Project](../yss-project/README.md) for transactions and external-file behavior.

Focused validation: `pnpm test:rs:package -p yss-project-model --lib`.
