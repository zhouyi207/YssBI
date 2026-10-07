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
`FileState` keeps the current content fingerprint with its private document; replacement
updates both together. Dirty-state metadata does not clone or encode the body.
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

`mind/query.rs` supplies a disposable `MindTree` over one validated document. Outline pages
use preorder traversal and the persisted sibling order; depth is relative to the selected root.
Search is a case-insensitive content substring within the selected subtree, with exact ancestor
paths. Targeted reads retain the requested ID order and reject missing or repeated IDs.
Queries borrow the original topics; they do not create persistent indexes or copy the whole body.

`mind/edit.rs` owns bulk addition, parent changes and subtree removal. Additions validate the
complete node set, so a child may precede its new parent in the batch. Bulk moves set all final
parents before applying sibling positions in request order, then validate the final tree.
`prepare_subtree_copy` allocates fresh IDs for disjoint source branches, preserves their contents
and external references, and returns the exact identity map and one insertion command.
Project continues to apply edits to an unpublished candidate and commits the whole file batch;
these model commands do not change persistence, dirty-state or document session ownership.

`doc/query.rs` borrows the current Markdown and derives a disposable Unicode location index.
The `pulldown-cmark` event parser supplies top-level ATX/setext headings and source block
boundaries; headings inside code, HTML, quoted blocks or list items do not become document sections.
Heading indices and character starts distinguish repeated titles. Sections include their heading
and descendants through the next heading of equal or shallower level. These locations describe
one captured document; the calling layer must retain its version when reusing them.
Reads preserve the original bytes and use Unicode scalar ranges, preferring a complete block
near the requested page end. Long blocks continue across pages. Literal, case-sensitive search
includes overlapping matches and returns exact character ranges and source-preserving context.
No Markdown AST or secondary document state is persisted.

Focused validation: `cargo test -p yss-project-model --lib`.
