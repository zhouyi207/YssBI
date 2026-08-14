# Sidebar Resource and Node Drag Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete regression coverage for draggable Graphs, Nodes, Variables, and Data sidebar rows while preserving the existing authoritative drag payload implementation.

**Architecture:** Sidebar rows continue to provide typed payloads through the shared `SidebarDraggableItem` and centralized DnD contracts. The existing production implementation already forwards backend-issued descriptors and uses a pointer cursor, so this plan adds the missing shared-shell regression test and verifies all existing payload paths without changing production behavior.

**Tech Stack:** TypeScript, React, dnd-kit, happy-dom, Vitest, pnpm

## Global Constraints

- Do not synthesize resource paths, revisions, ports, creation arguments, or node descriptors in React.
- Keep Workspace responsible for drag lifecycle and routing; do not add native HTML drag-and-drop or another drag framework.
- Preserve ordinary Function graph-resource drops and Shift-modified Call node creation.
- Keep descriptor-unavailable variable and database rows disabled with their existing refresh and warning behavior.
- Draggable sidebar rows use `cursor-pointer` and must not use `cursor-grab` or `active:cursor-grabbing`.
- Do not modify production code unless a focused regression test demonstrates that the checked-in implementation violates the approved design.
- Do not create Git commits unless explicitly requested.
- Run Vitest with `--pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1`.

---

### Task 1: Lock the shared draggable-row cursor contract

**Files:**
- Create: `src/views/EditorView/Layout/sidebarUi/SidebarDraggableItem.test.tsx`
- Verify: `src/views/EditorView/Layout/sidebarUi/SidebarDraggableItem.tsx`

**Interfaces:**
- Consumes: `SidebarDraggableItem({ id, dragData, children, dragDisabledReason? })` and dnd-kit's `useDraggable`.
- Produces: Regression coverage proving draggable rows use the pointer cursor and disabled rows do not expose draggable listeners or grab/grabbing cursor classes.

- [ ] **Step 1: Add a focused shared-shell test**

Create a happy-dom test that mocks `useDraggable`, renders one draggable row with this payload:

```ts
const dragData = {
  type: DRAG_TYPES.GRAPH_RESOURCE,
  sidebarResource: {
    id: 'functions/Revenue.yssbi-function',
    name: 'Revenue',
    type: 'function',
  },
} satisfies SidebarDragPayload;
```

Assert all of the following against the rendered row:

```ts
expect(row.classList.contains('cursor-pointer')).toBe(true);
expect(row.classList.contains('cursor-grab')).toBe(false);
expect(row.classList.contains('active:cursor-grabbing')).toBe(false);
expect(useDraggableInput).toEqual({
  id: 'sidebar-item-function-row',
  data: dragData,
  disabled: false,
});
expect(useDraggableInput.data).toBe(dragData);
```

Render a second row with `dragData={null}` and `dragDisabledReason="Descriptor unavailable"`; assert it has `aria-disabled="true"`, no pointer/grab/grabbing cursor class, and dnd-kit receives `disabled: true`.

- [ ] **Step 2: Run the new test**

Run:

```text
pnpm exec vitest run src/views/EditorView/Layout/sidebarUi/SidebarDraggableItem.test.tsx --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
```

Expected: both shared-shell cursor and disabled-state cases pass against the existing implementation.

- [ ] **Step 3: Keep production changes conditional**

If Step 2 fails because `SidebarDraggableItem` still uses grab/grabbing classes or forwards listeners while disabled, make only the minimal change in `SidebarDraggableItem.tsx` needed to satisfy the assertions. If it passes, leave production code unchanged.

- [ ] **Step 4: Re-run the focused test**

Run the Step 2 command again after any necessary correction. Expected: all tests pass.

---

### Task 2: Verify authoritative sidebar payload paths

**Files:**
- Verify: `src/views/EditorView/Layout/sidebar/tabs/sidebarEmptyStates.test.tsx`
- Verify: `src/views/EditorView/Layout/sidebar/rows/SidebarResourceRows.test.tsx`
- Create: `src/views/EditorView/Layout/sidebar/rows/SidebarGraphRow.test.tsx`
- Verify: `src/features/core/dnd/dndContracts.test.ts`
- Verify: `src/features/application/editor/sidebarSpawnDropPolicy.test.ts`
- Modify: `src/features/application/editor/handleGraphResourceDrop.test.ts`
- Verify: `src/features/application/editor/editorUnavailableRouting.test.tsx`

**Interfaces:**
- Consumes: Catalog `item.creation`, `buildSidebarDragData`, `SidebarDragPayload` guards, and existing editor drop routing.
- Produces: Evidence that Catalog nodes, Function resources, local/global variables, and databases retain their exact approved drag/drop semantics.

- [ ] **Step 1: Add explicit ordinary Function-drop coverage**

In `handleGraphResourceDrop.test.ts`, add a case that passes this Function resource to `handleGraphResourceDrop`:

```ts
const functionResource = {
  id: 'functions/Revenue.yssbi-function',
  name: 'Revenue',
  type: 'function' as const,
};
```

Assert that an ordinary merge drop calls:

```ts
expect(openGraphInEditor).toHaveBeenCalledWith(
  functionResource.id,
  functionResource.name,
  'function',
  'editor-b',
  { pinned: true },
);
```

- [ ] **Step 2: Run the payload and drop regression suite**

Run:

```text
pnpm exec vitest run src/views/EditorView/Layout/sidebar/tabs/sidebarEmptyStates.test.tsx src/views/EditorView/Layout/sidebar/rows/SidebarResourceRows.test.tsx src/views/EditorView/Layout/sidebar/rows/SidebarGraphRow.test.tsx src/features/core/dnd/dndContracts.test.ts src/features/application/editor/sidebarSpawnDropPolicy.test.ts src/features/application/editor/handleGraphResourceDrop.test.ts src/features/application/editor/editorUnavailableRouting.test.tsx --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
```

Expected coverage:

- Catalog rows register `node-template` payloads containing the exact `item.creation` descriptor and retain title/type rendering.
- Function rows retain graph-resource payloads.
- Local/global variables retain the exact current variable Get descriptor.
- Databases retain the exact current dataframe source descriptor.
- Missing descriptors keep rows disabled and preserve refresh behavior.
- Ordinary Function drops open graph resources and Shift-modified drops create Call nodes through existing routing.

- [ ] **Step 3: Run TypeScript validation**

Run:

```text
pnpm typecheck
```

Expected: exit code 0.

- [ ] **Step 4: Check the diff**

Run:

```text
git diff --check
```

Expected: exit code 0. The diff includes the shared cursor test, descriptor identity assertions, the Function row test, `handleGraphResourceDrop.test.ts`, and this plan; production files remain unchanged unless a focused test exposes a genuine approved-design violation.
