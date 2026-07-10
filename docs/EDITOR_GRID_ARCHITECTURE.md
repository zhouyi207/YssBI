# Editor Grid Architecture (GridWidget Evaluation)

## Conclusion

**No separate imperative `GridWidget` class is required.**

YssBI already implements the VS Code editor-part grid model with equivalent capabilities:

| VS Code | YssBI |
|---------|--------|
| `GridWidget` (row/col tree) | `layoutStore.nodes` under `editor_area` |
| `GridWidget.addView` | `splitEditorGroupInTree` |
| `GridWidget.removeView` | `removeEditorGroupFromTree` |
| `GridWidget.resizeView` | `sashResizeLogic` + `commitFlexSplitResize` |
| `SerializableGrid` | `editorGridMemento` |
| `IEditorGroupsService` | `EditorGroupsService` |
| DOM imperative render | `LayoutNodeRenderer` (React + flex) |

VS Code uses imperative DOM because the workbench is not React. YssBI’s React tree + zustand store achieves the same semantics without duplicating a second layout engine.

## When to revisit

Introduce a dedicated imperative grid only if:

- Nested row/col bugs become frequent and hard to test
- We need grid operations outside React (e.g. headless layout service)
- Performance profiling shows React reconciliation on grid structure changes is a bottleneck

Until then, extend **`editorGridLayout.ts`** (pure tree ops) and **`editorGridMemento.ts`** (persistence).

## Module boundaries

```
editorGridLayout.ts   — grid tree queries + mutations (GridWidget equivalent)
editorGridMemento.ts  — serialize / hydrate editor_area subtree
editorSplitLayout.ts  — edge → row/col placement
EditorGroupsService   — thin command facade for UI
layoutStore.ts        — zustand shell; delegates grid ops to editorGridLayout
LayoutNodeRenderer    — recursive row/col + Sash render
sashResizeLogic.ts    — imperative sash preview + commit
splitViewSizing.ts    — flex pair math (shared with workbench chrome)
```

## Persistence (Workbench vs Editor Grid)

Editor grid and workbench chrome share one localStorage key (`yssbi-workbench-layout`) but **logical slices** are decoupled:

| Slice | Schema field | Hydrate | Persist |
|-------|--------------|---------|---------|
| Chrome (sidebar/panel/detail) | `parts` | `hydrateWorkbenchChrome()` | `persistWorkbenchLayoutDebounced()` — merges `parts` only |
| Editor grid | `editorGrid` | `hydrateEditorGrid()` | `persistEditorGridDebounced()` — merges `editorGrid` only |
| Full reset | both | `hydrateWorkbenchLayout()` | `persistWorkbenchLayoutNow()` |

`mergeWorkbenchLayoutMemento()` in `workbenchLayoutPersistence.ts` patches one slice without overwriting the other.

**Project switch:** `collapseEditorGroupsForProjectSwitch()` collapses the in-memory grid and immediately persists a single-group `editorGrid` memento so refresh does not restore a stale split layout.

Sash drag commits call the appropriate persist function from `sashResizeLogic` (chrome vs grid sash). No duplicate sash-end listener.

## Deferred (not grid replacement)

- P2: drag tab to group edge merge (center drop)
- P2: auto-collapse empty group on last tab drag (partial — `removeEditorGroupFromTree` exists)
- P3: tab overflow menu
