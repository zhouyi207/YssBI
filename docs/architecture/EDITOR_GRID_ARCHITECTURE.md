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

## Drag & drop (VS Code `editorDropTarget`)

Tab / sidebar graph / **entire editor group** drag uses **pointer position** inside `data-editor-content`:

- Center dead zone (10% inset) → merge into group
- Edge bands → directional split via `editorSplitHitTest.ts` (33% zones; 30% when dragging a group)
- **Alt** (Win/Linux) / **Shift** (macOS) toggles `splitOnDragAndDrop` for the session
- **Ctrl** (Win/Linux) / **Alt** (macOS) → copy instead of move
- `openSideBySideDirection` biases split zones via `editorPartOptions.ts`
- Self-drop guard: same group + single tab hides split overlay

**Group drag:** tab-strip trailing `data-group-drag-fill` filler (VS Code empty `tabsContainer` gap).

**Drop surface:** `readEditorGroupDropBounds` — content below tab bar when tabs exist; full shell when empty (`getOverlayOffsetHeight`).

**Drag hover:** 1500ms over tab opens it (`tabBarDragHoverOpen.ts`, VS Code `DRAG_OVER_OPEN_TAB_THRESHOLD`).

**Close last tab:** `prepareActiveGroupBeforeLastTabClose` pre-activates MRU group before grid removal.

**Pointer resolve:** unified in `editorDropTarget.ts` (removed duplicates from drag monitor + tabBarInsertPreview).

TabBar chrome (VS Code `prepareEditorActions`):

- **Active** group (or `alwaysShowEditorActions`): split + close inline
- **Inactive** group: `…` overflow menu
- `pointerdown` on group shell / tab strip activates group before drag

Settings: `openSideBySideDirection`, `splitOnDragAndDrop`, `alwaysShowEditorActions`, `closeEmptyGroups`, `splitSizing` in `EditorSettings`.

Group ops: `mergeEditorGroup`, `splitEditorGroupWithGroup`, copy variants in `editorGroupCommands.ts`; MRU in `recentEditorGroupIds`.


Workbench chrome sashes and editor-grid sashes share `splitViewSizing.ts` / `sashResizeLogic.ts`, but commits are separate:

- **Chrome sash:** `resizePart` + `persistWorkbenchLayoutDebounced()`.
- **Grid sash:** adjacent editor-group pair resize + `persistEditorGridDebounced()`.

**VS Code–aligned split/close pipeline** (single sizing model under `editorGridSizing.ts`):

| Operation | Tree (`editorGridLayout`) | Sizing (`editorGridSizing`) |
|-----------|---------------------------|-----------------------------|
| `addView` / tab split | `splitEditorGroupInTree` | `applyEditorGridAddViewSizing` → halve target, then `commitEditorGridLayoutState` |
| `removeView` / close empty group | `removeEditorGroupFromTree` | `applyEditorGridRemoveViewSizing` (merge reference or distribute) → `commitEditorGridLayoutState` |
| Sash drag | — | `commitSplitPairSizes` (ratio `size` only) |
| Chrome/viewport change | — | `commitEditorGridLayoutState` |

`applyEditorGridAddViewSizing` mirrors VS Code `SplitView.addView` auto/split/distribute. **`auto`** checks only **siblings in the same row/col parent** (`areViewsDistributed`: max − min ≤ **2%** on `size` weights). Inserting a view **halves the target allocation** instead of inserting a default `size: 1`.

**`editorGridMemento` persists ratio weights only** — `computeEditorGridMementoSizes` derives weights from the live tree; hydrate restores viewport-independent flex ratios.

## Deferred (not grid replacement)

- P2: drag tab to group edge merge (center drop)
- P2: auto-collapse empty group on last tab drag (partial — `removeEditorGroupFromTree` exists)
- P3: tab overflow menu
