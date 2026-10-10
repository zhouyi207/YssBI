# Document editors reference

> Status: Historical
> Scope: Retained React Mind/Markdown editors and transient-input behavior
> Canonical owners: This module owns reference views only; current documents and use cases belong to Rust Project/Application
> Update when: Reference interactions or current contract routing changes

This directory does not participate in native builds. Current entry points are the
[root README](../../../../README.md), [documentation index](../../../../docs/README.md),
[GPUI host](../../../../crates/yss-desktop-gpui/README.md),
[Project model](../../../../crates/yss-project-model/README.md) and
[Project application](../../../../crates/yss-application/src/project/README.md).
The behaviors below describe reference source, not completed native migration.

## Mind behavior reference

- Mind and Markdown use separate typed snapshots and edit operations. Shared loading,
  save and close mechanics do not combine their document models.
- The reference Mind canvas derives a deterministic tree layout from parent links,
  sibling order and collapsed branches. Coordinates are disposable presentation state,
  not editable or persisted document content.
- Selection and collapsed branches belong to a pane. Split panes share the document
  but retain independent selection; a single selected topic appears in Details,
  while empty or multiple selection shows file information.
- Text, parent, sibling order and add/delete actions use Details. Root deletion is
  blocked, and deleting an ancestor covers its descendants in the same batch.
- Escape cancels a gesture and restores its start state; otherwise it clears selection.
  Hidden or replaced panes reject late pointer events. Input fields retain their own shortcuts.
- The reference editor does not expose a resource-reference picker, Mermaid import/export
  or document-level Mind undo/redo. These limits do not define native feature coverage.

## Markdown behavior reference

- Editing and preview share the current input buffer. Switching modes preserves unfinished
  input; preview supports GFM tables and math without changing the saved text.
- Single tildes in model notation remain literal; double tildes delimit strikethrough.
  Inline and display math follow parsed delimiters, leaving code and escaped dollars literal.
- Wide tables, code and display formulas scroll within the reading pane. Unknown code
  languages stay readable without requiring successful syntax highlighting.
- Links are explicit actions, not unrestricted navigation. Remote reference previews do
  not create project resources or document drafts; preview failure must not claim success.
- Report-writing conventions belong to the current
  [Harness](../../../../crates/yss-harness-core/README.md), not heuristics in a renderer.

## Input and resource lifecycle reference

- Input buffers capture a base editing version and flush on editing completion or explicit
  Save. Concurrent inputs report stale-version conflicts rather than silently overwriting.
- Mind topic inputs survive a Details target switch. Last-panel close, explicit removal
  and project replacement release only their own buffers; rename moves retained inputs.
- External disappearance preserves dirty input until explicit discard, without restoring
  the resource's index membership.
- Initial reads are shared by matching project/file identity. Close, rename, project
  replacement or newer revisions invalidate late replies.
- Save-all and Save/Discard/Cancel wait for already submitted edits. Pending operations
  live in the edit queue, not in a second UI revision model.

See [resource operations](../../features/application/resource/README.md) for the related
reference behavior.

## Open manual acceptance

These remain comparison scenarios, not passed native checks:

- Create, rename, duplicate, remove, save and reopen Mind/Doc resources from menu and sidebar.
- Mind selection, root protection, reparenting, sibling order, collapse and split-pane isolation.
- Markdown input/preview switching, tables, inline/display math, code, narrow panes and scrolling.
- Save before blur, stale-input rejection, external removal and Save/Discard/Cancel.
- Project switching and renaming during pending reads; late replies must not restore old resources.
- Assistant edits alongside unfinished UI input and explicit external-reference actions.

Current migration scope and outstanding native acceptance are maintained in the
[migration plan](../../../../docs/roadmap/GPUI_MIGRATION.md).
