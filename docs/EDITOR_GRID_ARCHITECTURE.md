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

Editor grid and workbench chrome share one localStorage key scoped by **Tauri window label** (`yssbi-workbench-layout:<label>`; main window uses the default scope). Logical slices stay decoupled:

| Slice | Schema field | Hydrate | Persist |
|-------|--------------|---------|---------|
| Chrome (sidebar/panel/detail) | `parts` | `hydrateWorkbenchChrome()` | `persistWorkbenchLayoutDebounced()` — snapshots chrome at schedule time; merges `parts` only |
| Editor grid | `editorGrid` | `hydrateEditorGrid()` | `persistEditorGridDebounced()` — merges `editorGrid` only |
| Full reset | chrome only | n/a | `resetWorkbenchLayout()` → `persistWorkbenchLayoutNow()` |

`mergeWorkbenchLayoutMemento()` in `workbenchLayoutPersistence.ts` uses a slice-aware pending queue: parts and editorGrid scheduled in the same debounce window are merged, not overwritten.

**Reset Layout semantics:** `resetWorkbenchLayout()` restores default chrome visibility, sizes, panel position, and maximize state only. It does **not** collapse editor groups, close tabs, or change `activeEditorGroupId`.

**Zen mode:** chrome toggles and chrome persistence are no-ops while Zen is active; viewport reclamp may still update in-memory panel size but does not schedule chrome writes.

**Project switch:** `collapseEditorGroupsForProjectSwitch()` collapses the in-memory grid and immediately persists a single-group `editorGrid` memento so refresh does not restore a stale split layout.

## Editor Grid pair sizing

Workbench chrome sashes and editor-grid sashes share `splitViewSizing.ts` / `sashResizeLogic.ts`, but commits are separate:

- **Chrome sash:** `resizePart` + `persistWorkbenchLayoutDebounced()`.
- **Grid sash:** adjacent editor-group pair resize (including after both leaves are pixelized) + `persistEditorGridDebounced()`.

Flex-only grid splits are pixelized on first drag via `commitFlexSplitResize`; persisted `pixelSize` values restore split ratios on hydrate.

## Deferred (not grid replacement)

- P2: drag tab to group edge merge (center drop)
- P2: auto-collapse empty group on last tab drag (partial — `removeEditorGroupFromTree` exists)
- P3: tab overflow menu
