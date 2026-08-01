# Sidebar Empty States Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Sidebar empty rows with explicit tab-level and section-level empty-state models so empty tabs never mount unnecessary scrollbars while resource lists retain virtualization.

**Architecture:** `features/core/sidebar` will produce structured `SidebarPanelModel` and `SidebarTreeModel` values without presentation-only empty rows. `views/EditorView/Layout/sidebar` will flatten panel models into private virtual render rows and render tab-level states outside `OverlayScrollbar`. Shared resource stores, Sidebar actions, and `OverlayScrollbar` remain unchanged.

**Tech Stack:** React 19, TypeScript 5.8, Zustand, TanStack React Virtual, shadcn/Radix Tooltip, Tailwind CSS, i18next, Vitest, happy-dom

## Global Constraints

- Follow `docs/superpowers/specs/2026-08-01-sidebar-empty-states-design.md` as the approved behavior contract.
- Keep Rust and React separated; this plan contains frontend changes only.
- Domain/core Sidebar model code must not import React components, services, views, or framework state.
- User-facing vertical scrolling must continue to use `src/shared/ui/OverlayScrollbar.tsx`.
- Do not modify `OverlayScrollbar` to special-case empty states.
- Preserve Sidebar section expansion, add actions, selection, context menus, resource opening, and long-list virtualization.
- All new user-facing strings must exist in both `src/app/i18n/locales/zh-CN.ts` and `src/app/i18n/locales/en-US.ts`.
- Add focused regression tests before changing each behavior.
- Run project commands from the repository root through `pnpm` scripts.
- Do not create commits unless the user explicitly asks for them.

---

## File Structure

### Core model and builders

- Create `src/features/core/sidebar/flatRows/sidebarPanelModel.ts`: framework-independent empty-state, section, panel, and tree model types.
- Create `src/features/core/sidebar/flatRows/sidebarPanelModel.test.ts`: builder contract tests covering empty and populated sections plus node search results.
- Modify `src/features/core/sidebar/flatRows/types.ts`: replace `FlatSidebarRow` with item-only `SidebarItemRow`; remove `section` and `empty` presentation variants.
- Delete `src/features/core/sidebar/flatRows/appendSectionBlock.ts`: section flattening moves to the View layer.
- Modify `src/features/core/sidebar/flatRows/buildGraphsFlatRows.ts`: export `buildGraphsSidebarModel()` returning structured sections.
- Modify `src/features/core/sidebar/flatRows/buildVariablesFlatRows.ts`: export `buildVariablesSidebarModel()`.
- Modify `src/features/core/sidebar/flatRows/buildDataFlatRows.ts`: export `buildDataSidebarModel()`.
- Modify `src/features/core/sidebar/flatRows/buildChartsFlatRows.ts`: export `buildChartsSidebarModel()`.
- Modify `src/features/core/sidebar/flatRows/buildNodesFlatRows.ts`: export `buildNodesSidebarModel()` with a tab-level no-match state.
- Modify `src/features/core/sidebar/flatRows/index.ts` and `src/features/core/sidebar/index.ts`: export the new model types and builder names; stop exporting removed APIs.

### View rendering

- Create `src/views/EditorView/Layout/sidebar/sections/sidebarRenderRows.ts`: private `SidebarRenderRow` type and pure `flattenSidebarPanelModel()` adapter.
- Create `src/views/EditorView/Layout/sidebar/sections/sidebarRenderRows.test.ts`: adapter tests for expanded, collapsed, populated, and empty sections.
- Create `src/views/EditorView/Layout/sidebar/sections/SidebarEmptyState.tsx`: tab-level empty-state component.
- Create `src/views/EditorView/Layout/sidebar/sections/SidebarSectionEmptyState.tsx`: compact section-level empty-state component with Tooltip.
- Create `src/views/EditorView/Layout/sidebar/sections/sidebarEmptyStateComponents.test.tsx`: component layout and no-scrollbar tests.
- Modify `src/views/EditorView/Layout/sidebar/sections/SidebarFlatRowPanel.tsx`: accept `SidebarPanelModel`, flatten it in the View layer, and retain existing action context.
- Modify `src/views/EditorView/Layout/sidebar/sections/SidebarFlatRowList.tsx`: consume private `SidebarRenderRow[]`.
- Modify `src/views/EditorView/Layout/sidebar/sections/SidebarFlatRowItem.tsx`: render `section`, `sectionEmpty`, and item rows; remove the old core `empty` branch.

### Tab integration and localization

- Create `src/views/EditorView/Layout/sidebar/tabs/sidebarEmptyStates.test.tsx`: regressions for unavailable Nodes and inactive Commands tabs.
- Modify `src/views/EditorView/Layout/sidebar/tabs/SidebarGraphsTab.tsx`.
- Modify `src/views/EditorView/Layout/sidebar/tabs/SidebarVariablesTab.tsx`.
- Modify `src/views/EditorView/Layout/sidebar/tabs/SidebarDataTab.tsx`.
- Modify `src/views/EditorView/Layout/sidebar/tabs/SidebarChartsTab.tsx`.
- Modify `src/views/EditorView/Layout/sidebar/tabs/SidebarNodesTab.tsx`.
- Modify `src/views/EditorView/Layout/sidebar/tabs/SidebarCommandsTab.tsx`.
- Modify `src/app/i18n/locales/zh-CN.ts` and `src/app/i18n/locales/en-US.ts`.

---

### Task 1: Introduce Structured Sidebar Models

**Files:**
- Create: `src/features/core/sidebar/flatRows/sidebarPanelModel.ts`
- Create: `src/features/core/sidebar/flatRows/sidebarPanelModel.test.ts`
- Modify: `src/features/core/sidebar/flatRows/types.ts`
- Modify: `src/features/core/sidebar/flatRows/buildGraphsFlatRows.ts`
- Modify: `src/features/core/sidebar/flatRows/buildVariablesFlatRows.ts`
- Modify: `src/features/core/sidebar/flatRows/buildDataFlatRows.ts`
- Modify: `src/features/core/sidebar/flatRows/buildChartsFlatRows.ts`
- Modify: `src/features/core/sidebar/flatRows/buildNodesFlatRows.ts`
- Modify: `src/features/core/sidebar/flatRows/index.ts`
- Modify: `src/features/core/sidebar/index.ts`
- Delete: `src/features/core/sidebar/flatRows/appendSectionBlock.ts`

**Interfaces:**
- Produces: `SidebarEmptyStateModel`, `SidebarSectionModel`, `SidebarPanelModel`, `SidebarTreeModel`, and `SidebarItemRow`.
- Produces: `buildGraphsSidebarModel`, `buildVariablesSidebarModel`, `buildDataSidebarModel`, `buildChartsSidebarModel`, and `buildNodesSidebarModel`.
- Consumes: `SidebarSectionKey`, existing resource records, existing translated labels, and existing group expansion helpers.

- [ ] **Step 1: Write failing structured-model tests**

Create `src/features/core/sidebar/flatRows/sidebarPanelModel.test.ts` with direct pure-function tests. Use representative empty and populated inputs:

```ts
import { describe, expect, it } from 'vitest';
import {
  buildChartsSidebarModel,
  buildDataSidebarModel,
  buildGraphsSidebarModel,
  buildNodesSidebarModel,
  buildVariablesSidebarModel,
} from './index';

describe('structured Sidebar models', () => {
  it('keeps empty graph sections as model metadata instead of rows', () => {
    const model = buildGraphsSidebarModel({
      events: {},
      functions: {},
      expandedSections: { graphsEvent: true, graphsFunction: false },
      labels: {
        event: 'Event',
        function: 'Function',
        noEvents: 'No events',
        noFunctions: 'No functions',
      },
    });

    expect(model.sections).toEqual([
      {
        key: 'graphsEvent',
        label: 'Event',
        expanded: true,
        rows: [],
        emptyMessage: 'No events',
      },
      {
        key: 'graphsFunction',
        label: 'Function',
        expanded: false,
        rows: [],
        emptyMessage: 'No functions',
      },
    ]);
  });

  it('stores graph resources only as item rows', () => {
    const model = buildGraphsSidebarModel({
      events: { 'events/Main.yssbi-event': { name: 'Main' } },
      functions: {},
      expandedSections: {},
      labels: {
        event: 'Event',
        function: 'Function',
        noEvents: 'No events',
        noFunctions: 'No functions',
      },
    });

    expect(model.sections[0].rows).toEqual([
      {
        kind: 'graph',
        rowKey: 'graph:event:events/Main.yssbi-event',
        level: 1,
        id: 'events/Main.yssbi-event',
        name: 'Main',
        graphType: 'event',
      },
    ]);
    expect(model.sections[0].rows.map((row) => row.kind)).toEqual(['graph']);
  });

  it('represents an unmatched node search as a tab-level empty state', () => {
    const model = buildNodesSidebarModel({
      items: [],
      filterQuery: 'missing',
      expandedGroups: {},
      noMatchesMessage: 'No matching nodes',
    });

    expect(model).toEqual({
      rows: [],
      emptyState: { title: 'No matching nodes' },
    });
  });

  it('builds empty data, chart, and variable sections without empty item rows', () => {
    expect(
      buildDataSidebarModel({
        dataframes: {},
        expandedSections: {},
        labels: { data: 'Data', noData: 'No data' },
      }).sections[0].rows,
    ).toEqual([]);

    expect(
      buildChartsSidebarModel({
        worksheets: [],
        expandedSections: {},
        labels: { worksheets: 'Worksheets', noWorksheets: 'No worksheets' },
      }).sections[0].rows,
    ).toEqual([]);

    const variables = buildVariablesSidebarModel({
      localVariables: {},
      globalVariables: {},
      hasActiveGraph: false,
      expandedSections: {},
      labels: {
        local: 'Local',
        global: 'Global',
        noLocal: 'No local variables',
        noGlobal: 'No global variables',
        noActiveGraph: 'No active graph',
      },
    });
    expect(variables.sections.map((section) => section.emptyMessage)).toEqual([
      'No active graph',
      'No global variables',
    ]);
  });
});
```

- [ ] **Step 2: Run the model test and verify it fails**

Run:

```sh
pnpm test -- src/features/core/sidebar/flatRows/sidebarPanelModel.test.ts
```

Expected: FAIL because the new builder names and model types do not exist.

- [ ] **Step 3: Define item-only and structured model types**

In `types.ts`, rename `FlatSidebarRow` to `SidebarItemRow` and retain only these variants:

```ts
export type SidebarItemRow =
  | SidebarGroupItemRow
  | SidebarGraphItemRow
  | SidebarVariableItemRow
  | SidebarDatabaseItemRow
  | SidebarWorksheetItemRow
  | SidebarNodeItemRow;
```

Do not include `section`, `empty`, or `sectionEmpty` in this core type.

Create `sidebarPanelModel.ts`:

```ts
import type { SidebarSectionKey } from '../sidebarSectionState';
import type { SidebarItemRow } from './types';

export interface SidebarEmptyStateModel {
  title: string;
  description?: string;
  action?: {
    label: string;
    command: string;
  };
}

export interface SidebarSectionModel {
  key: SidebarSectionKey;
  label: string;
  expanded: boolean;
  rows: SidebarItemRow[];
  emptyMessage?: string;
}

export interface SidebarPanelModel {
  sections: SidebarSectionModel[];
  emptyState?: SidebarEmptyStateModel;
}

export interface SidebarTreeModel {
  rows: SidebarItemRow[];
  emptyState?: SidebarEmptyStateModel;
}
```

- [ ] **Step 4: Convert section builders to return models**

Rename each exported builder and return `{ sections: [...] }`. Resolve expansion in the builder but do not flatten headers or empty messages into rows. For example, `buildGraphsSidebarModel()` must return:

```ts
return {
  sections: [
    {
      key: 'graphsEvent',
      label: params.labels.event,
      expanded: resolveSectionExpanded(params.expandedSections, 'graphsEvent'),
      rows: eventItems,
      emptyMessage: params.labels.noEvents,
    },
    {
      key: 'graphsFunction',
      label: params.labels.function,
      expanded: resolveSectionExpanded(params.expandedSections, 'graphsFunction'),
      rows: functionItems,
      emptyMessage: params.labels.noFunctions,
    },
  ],
};
```

Apply the same pattern to Variables, Data, and Charts. Preserve all current row keys, levels, IDs, names, data payloads, and variable scope flags.

Rename `buildNodesFlatRows()` to `buildNodesSidebarModel()`. Return `{ rows }` for populated results and return this exact no-match state when a non-empty query has no results:

```ts
return {
  rows: [],
  emptyState: { title: params.noMatchesMessage },
};
```

When there are no catalog items and no query, return `{ rows: [] }`; unavailable catalog messaging remains the responsibility of `SidebarNodesTab`.

- [ ] **Step 5: Update exports and remove obsolete flattening**

Update both barrel files to export the new names and types. Delete `appendSectionBlock.ts` and remove all imports/exports of `appendSectionBlock`, `FlatSidebarRow`, and the old `build*FlatRows` names.

- [ ] **Step 6: Run the focused model test**

Run:

```sh
pnpm test -- src/features/core/sidebar/flatRows/sidebarPanelModel.test.ts
```

Expected: PASS with four tests.

- [ ] **Step 7: Run TypeScript to expose all required call-site migrations**

Run:

```sh
pnpm typecheck
```

Expected: FAIL only in Sidebar View files that still import `FlatSidebarRow` or old builder names. Record those errors as the migration list for Tasks 2 and 3; do not add compatibility aliases.

---

### Task 2: Add View-Layer Render Rows and Empty-State Components

**Files:**
- Create: `src/views/EditorView/Layout/sidebar/sections/sidebarRenderRows.ts`
- Create: `src/views/EditorView/Layout/sidebar/sections/sidebarRenderRows.test.ts`
- Create: `src/views/EditorView/Layout/sidebar/sections/SidebarEmptyState.tsx`
- Create: `src/views/EditorView/Layout/sidebar/sections/SidebarSectionEmptyState.tsx`
- Create: `src/views/EditorView/Layout/sidebar/sections/sidebarEmptyStateComponents.test.tsx`
- Modify: `src/views/EditorView/Layout/sidebar/sections/SidebarFlatRowPanel.tsx`
- Modify: `src/views/EditorView/Layout/sidebar/sections/SidebarFlatRowList.tsx`
- Modify: `src/views/EditorView/Layout/sidebar/sections/SidebarFlatRowItem.tsx`

**Interfaces:**
- Consumes: `SidebarPanelModel`, `SidebarItemRow`, `SidebarSectionKey`, existing `SidebarGroupRow`, and existing row components.
- Produces: private `SidebarRenderRow` and `flattenSidebarPanelModel(model: SidebarPanelModel): SidebarRenderRow[]`.
- Produces: `SidebarEmptyState` and `SidebarSectionEmptyState`.
- Produces: `SidebarFlatRowPanel` prop `model: SidebarPanelModel`, replacing `rows: FlatSidebarRow[]`.

- [ ] **Step 1: Write failing adapter tests**

Create `sidebarRenderRows.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import type { SidebarPanelModel } from '@/features/core/sidebar';
import { flattenSidebarPanelModel } from './sidebarRenderRows';

const emptyGraphs: SidebarPanelModel = {
  sections: [
    {
      key: 'graphsEvent',
      label: 'Event',
      expanded: true,
      rows: [],
      emptyMessage: 'No events',
    },
    {
      key: 'graphsFunction',
      label: 'Function',
      expanded: false,
      rows: [],
      emptyMessage: 'No functions',
    },
  ],
};

describe('flattenSidebarPanelModel', () => {
  it('emits section empty rows only for expanded empty sections', () => {
    expect(flattenSidebarPanelModel(emptyGraphs)).toEqual([
      {
        kind: 'section',
        rowKey: 'section:graphsEvent',
        sectionKey: 'graphsEvent',
        level: 0,
        label: 'Event',
        expanded: true,
      },
      {
        kind: 'sectionEmpty',
        rowKey: 'section-empty:graphsEvent',
        sectionKey: 'graphsEvent',
        level: 1,
        message: 'No events',
      },
      {
        kind: 'section',
        rowKey: 'section:graphsFunction',
        sectionKey: 'graphsFunction',
        level: 0,
        label: 'Function',
        expanded: false,
      },
    ]);
  });

  it('places populated rows after their expanded section header', () => {
    const model: SidebarPanelModel = {
      sections: [
        {
          key: 'dataData',
          label: 'Data',
          expanded: true,
          emptyMessage: 'No data',
          rows: [
            {
              kind: 'database',
              rowKey: 'database:db-1',
              level: 1,
              id: 'db-1',
              name: 'Sales',
              data: { name: 'Sales' },
            },
          ],
        },
      ],
    };

    expect(flattenSidebarPanelModel(model).map((row) => row.kind)).toEqual([
      'section',
      'database',
    ]);
  });

  it('does not synthesize a placeholder without an empty message', () => {
    const model: SidebarPanelModel = {
      sections: [{ key: 'dataData', label: 'Data', expanded: true, rows: [] }],
    };
    expect(flattenSidebarPanelModel(model)).toHaveLength(1);
  });
});
```

- [ ] **Step 2: Run the adapter test and verify it fails**

Run:

```sh
pnpm test -- src/views/EditorView/Layout/sidebar/sections/sidebarRenderRows.test.ts
```

Expected: FAIL because `sidebarRenderRows.ts` does not exist.

- [ ] **Step 3: Implement the private render-row adapter**

Create `sidebarRenderRows.ts` with View-only section variants:

```ts
import type {
  SidebarItemRow,
  SidebarPanelModel,
  SidebarSectionKey,
} from '@/features/core/sidebar';

export type SidebarRenderRow =
  | SidebarItemRow
  | {
      kind: 'section';
      rowKey: string;
      sectionKey: SidebarSectionKey;
      level: 0;
      label: string;
      expanded: boolean;
    }
  | {
      kind: 'sectionEmpty';
      rowKey: string;
      sectionKey: SidebarSectionKey;
      level: 1;
      message: string;
    };

export function flattenSidebarPanelModel(model: SidebarPanelModel): SidebarRenderRow[] {
  return model.sections.flatMap((section) => {
    const header: SidebarRenderRow = {
      kind: 'section',
      rowKey: `section:${section.key}`,
      sectionKey: section.key,
      level: 0,
      label: section.label,
      expanded: section.expanded,
    };
    if (!section.expanded) return [header];
    if (section.rows.length > 0) return [header, ...section.rows];
    if (!section.emptyMessage) return [header];
    return [
      header,
      {
        kind: 'sectionEmpty',
        rowKey: `section-empty:${section.key}`,
        sectionKey: section.key,
        level: 1,
        message: section.emptyMessage,
      },
    ];
  });
}
```

If `SidebarSectionKey` is not re-exported from the same barrel as the model types, import it from its existing public Sidebar barrel; do not import from a View module.

- [ ] **Step 4: Run the adapter test**

Run:

```sh
pnpm test -- src/views/EditorView/Layout/sidebar/sections/sidebarRenderRows.test.ts
```

Expected: PASS with three tests.

- [ ] **Step 5: Write failing component tests**

Create `sidebarEmptyStateComponents.test.tsx` using `createRoot` and `act`:

```tsx
// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { TooltipProvider } from '@/components/ui/tooltip';
import { SidebarEmptyState } from './SidebarEmptyState';
import { SidebarSectionEmptyState } from './SidebarSectionEmptyState';

describe('Sidebar empty-state components', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('renders a wrapping tab-level state without a scrollbar viewport', () => {
    act(() => {
      root.render(
        <SidebarEmptyState
          title="Node catalog unavailable"
          description="Waiting for stable catalog descriptors"
        />,
      );
    });

    expect(host.textContent).toContain('Node catalog unavailable');
    expect(host.textContent).toContain('Waiting for stable catalog descriptors');
    expect(host.querySelector('.overlay-scrollbar-viewport')).toBeNull();
    expect(host.firstElementChild?.className).toContain('px-3');
  });

  it('renders a compact truncated section state with the full accessible label', () => {
    act(() => {
      root.render(
        <TooltipProvider>
          <SidebarSectionEmptyState
            level={1}
            message="A deliberately long section empty-state message"
          />
        </TooltipProvider>,
      );
    });

    const message = host.querySelector('[aria-label="A deliberately long section empty-state message"]');
    expect(message).not.toBeNull();
    expect(message?.className).toContain('truncate');
    expect(message?.closest('.h-7')).not.toBeNull();
  });
});
```

- [ ] **Step 6: Run the component test and verify it fails**

Run:

```sh
pnpm test -- src/views/EditorView/Layout/sidebar/sections/sidebarEmptyStateComponents.test.tsx
```

Expected: FAIL because both components do not exist.

- [ ] **Step 7: Implement both empty-state components**

Implement `SidebarEmptyState.tsx` as a presentation-only component:

```tsx
import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';

export function SidebarEmptyState({
  title,
  description,
  action,
  className,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn('min-w-0 px-3 py-3 text-xs', className)}>
      <div className="break-words text-foreground/85">{title}</div>
      {description ? (
        <div className="mt-1 break-words leading-relaxed text-muted-foreground">
          {description}
        </div>
      ) : null}
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  );
}
```

Implement `SidebarSectionEmptyState.tsx` with the existing indentation helper and shadcn Tooltip:

```tsx
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { sidebarItemIndent } from '../../sidebarUi/sidebarStyles';

export function SidebarSectionEmptyState({
  level,
  message,
  onContextMenu,
}: {
  level: number;
  message: string;
  onContextMenu?: (event: React.MouseEvent) => void;
}) {
  return (
    <div
      className="flex h-7 w-full min-w-0 items-center pr-2 text-[12px] text-muted-foreground/70"
      style={sidebarItemIndent(level)}
      onContextMenu={onContextMenu}
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <span className="block min-w-0 flex-1 truncate" aria-label={message}>
            {message}
          </span>
        </TooltipTrigger>
        <TooltipContent side="right">{message}</TooltipContent>
      </Tooltip>
    </div>
  );
}
```

- [ ] **Step 8: Migrate the virtual-list components**

Change `SidebarFlatRowPanel` to accept `model: SidebarPanelModel`, derive rows with `useMemo(() => flattenSidebarPanelModel(model), [model])`, and pass them to `SidebarFlatRowList`. Keep the current context value and handler refs unchanged.

Change `SidebarFlatRowList` to accept `SidebarRenderRow[]`. Keep `SIDEBAR_FLAT_ROW_HEIGHT`, overscan, absolute positioning, one shared `OverlayScrollbar`, and the zero-row fast path unchanged.

Change `SidebarFlatRowItem` to accept `SidebarRenderRow`. Remove the local `SidebarEmptyRow`. Render `sectionEmpty` with:

```tsx
<SidebarSectionEmptyState
  level={row.level}
  message={row.message}
  onContextMenu={ctx.sectionActions[row.sectionKey]?.onContentContextMenu}
/>
```

Keep the existing exhaustive `never` check.

- [ ] **Step 9: Run View-layer focused tests**

Run:

```sh
pnpm test -- src/views/EditorView/Layout/sidebar/sections/sidebarRenderRows.test.ts src/views/EditorView/Layout/sidebar/sections/sidebarEmptyStateComponents.test.tsx
```

Expected: PASS with five tests.

---

### Task 3: Integrate Every Sidebar Tab and Localize Empty States

**Files:**
- Create: `src/views/EditorView/Layout/sidebar/tabs/sidebarEmptyStates.test.tsx`
- Modify: `src/views/EditorView/Layout/sidebar/tabs/SidebarGraphsTab.tsx`
- Modify: `src/views/EditorView/Layout/sidebar/tabs/SidebarVariablesTab.tsx`
- Modify: `src/views/EditorView/Layout/sidebar/tabs/SidebarDataTab.tsx`
- Modify: `src/views/EditorView/Layout/sidebar/tabs/SidebarChartsTab.tsx`
- Modify: `src/views/EditorView/Layout/sidebar/tabs/SidebarNodesTab.tsx`
- Modify: `src/views/EditorView/Layout/sidebar/tabs/SidebarCommandsTab.tsx`
- Modify: `src/app/i18n/locales/zh-CN.ts`
- Modify: `src/app/i18n/locales/en-US.ts`

**Interfaces:**
- Consumes: `buildGraphsSidebarModel`, `buildVariablesSidebarModel`, `buildDataSidebarModel`, `buildChartsSidebarModel`, `SidebarFlatRowPanel model`, and `SidebarEmptyState`.
- Preserves: all existing section action maps and resource handlers.
- Produces: localized tab-level Nodes and Commands empty states.

- [ ] **Step 1: Add the localized strings**

Add these keys under `sidebar` in both locales:

```ts
// zh-CN.ts
nodeCatalogUnavailable: '节点目录暂不可用',
nodeCatalogUnavailableDescription: '等待稳定的节点目录描述信息',
nodeSearchNoMatches: '未找到匹配的节点',
noActiveGraph: '未打开活动图',
noActiveGraphDescription: '打开一个 Event 或 Function 后可查看命令状态',
noData: '暂无数据',
noEvents: '暂无 Event',
noFunctions: '暂无 Function',
noLocalVariables: '暂无局部变量',
noGlobalVariables: '暂无全局变量',
```

```ts
// en-US.ts
nodeCatalogUnavailable: 'Node catalog unavailable',
nodeCatalogUnavailableDescription: 'Waiting for stable node catalog descriptors',
nodeSearchNoMatches: 'No matching nodes',
noActiveGraph: 'No active graph open',
noActiveGraphDescription: 'Open an Event or Function to view command status',
noData: 'No data yet',
noEvents: 'No Events yet',
noFunctions: 'No Functions yet',
noLocalVariables: 'No local variables yet',
noGlobalVariables: 'No global variables yet',
```

Update the existing `chartsSidebar.noWorksheets` translations to `暂无工作表` and `No worksheets yet` rather than introducing a duplicate Sidebar key.

- [ ] **Step 2: Write failing Nodes and Commands regression tests**

Create `sidebarEmptyStates.test.tsx`. Mock `react-i18next` to return keys or a small deterministic translation map, and mock `useEditorHistoryAvailability` for the Commands test. Render each tab into a real DOM root:

```tsx
// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const historyAvailability = vi.hoisted(() => ({
  activeTabId: null as string | null,
  canUndo: false,
  canRedo: false,
  pending: false,
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) =>
      ({
        'sidebar.nodeCatalogUnavailable': 'Node catalog unavailable',
        'sidebar.nodeCatalogUnavailableDescription': 'Waiting for descriptors',
        'sidebar.noActiveGraph': 'No active graph open',
        'sidebar.noActiveGraphDescription': 'Open a graph to view commands',
        'common.undo': 'Undo',
        'common.redo': 'Redo',
      })[key] ?? key,
  }),
}));

vi.mock('@/features/application/editor', () => ({
  useEditorHistoryAvailability: () => historyAvailability,
}));

import { SidebarCommandsTab } from './SidebarCommandsTab';
import { SidebarNodesTab } from './SidebarNodesTab';

describe('Sidebar tab-level empty states', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('does not mount a scroll viewport for an unavailable node catalog', () => {
    act(() => root.render(<SidebarNodesTab />));
    expect(host.textContent).toContain('Node catalog unavailable');
    expect(host.textContent).toContain('Waiting for descriptors');
    expect(host.querySelector('.overlay-scrollbar-viewport')).toBeNull();
  });

  it('uses the shared empty state when Commands has no active graph', () => {
    historyAvailability.activeTabId = null;
    act(() => root.render(<SidebarCommandsTab />));
    expect(host.textContent).toContain('No active graph open');
    expect(host.textContent).toContain('Open a graph to view commands');
    expect(host.querySelector('.overlay-scrollbar-viewport')).toBeNull();
  });

  it('keeps command controls when an active graph exists', () => {
    historyAvailability.activeTabId = 'events/Main.yssbi-event';
    act(() => root.render(<SidebarCommandsTab />));
    expect(host.textContent).toContain('Undo');
    expect(host.textContent).toContain('Redo');
  });
});
```

- [ ] **Step 3: Run the tab regression test and verify it fails**

Run:

```sh
pnpm test -- src/views/EditorView/Layout/sidebar/tabs/sidebarEmptyStates.test.tsx
```

Expected: FAIL because Nodes still mounts `SidebarFlatRowPanel` and both tabs still use old text paths.

- [ ] **Step 4: Migrate section-based tabs**

In Graphs, Variables, Data, and Charts:

1. Replace old builder imports with the corresponding `build*SidebarModel` name.
2. Rename local `rows` variables to `model`.
3. Keep each existing `useMemo` dependency list complete.
4. Pass `model={model}` to `SidebarFlatRowPanel`.
5. Keep every `sectionActions`, `graphIssueCounts`, toggle handler, resource handler, and context-menu handler unchanged.

Example Graphs shape:

```tsx
const model = useMemo(
  () => buildGraphsSidebarModel({ /* existing inputs and labels */ }),
  [events, functions, sectionExpanded, t],
);

<SidebarFlatRowPanel
  model={model}
  sectionActions={sectionActions}
  graphIssueCounts={graphIssueCounts}
  onToggleSection={toggleSection}
  onToggleGroup={noopSidebarHandler}
  onGraphContextMenu={onGraphContextMenu}
/>
```

- [ ] **Step 5: Migrate tab-level empty states**

Replace `SidebarNodesTab` content with localized `SidebarEmptyState`:

```tsx
export function SidebarNodesTab() {
  const { t } = useTranslation();
  return (
    <SidebarTabPanel>
      <SidebarEmptyState
        title={t('sidebar.nodeCatalogUnavailable')}
        description={t('sidebar.nodeCatalogUnavailableDescription')}
      />
    </SidebarTabPanel>
  );
}
```

Remove its import of `NODE_CATALOG_UNAVAILABLE_MESSAGE`, `SidebarFlatRowPanel`, and no-op toggle callbacks.

In `SidebarCommandsTab`, replace the no-active-graph `div` with:

```tsx
<SidebarEmptyState
  title={t('sidebar.noActiveGraph')}
  description={t('sidebar.noActiveGraphDescription')}
/>
```

Do not change active Commands rendering.

- [ ] **Step 6: Run the focused Sidebar test set**

Run:

```sh
pnpm test -- src/features/core/sidebar/flatRows/sidebarPanelModel.test.ts src/views/EditorView/Layout/sidebar/sections/sidebarRenderRows.test.ts src/views/EditorView/Layout/sidebar/sections/sidebarEmptyStateComponents.test.tsx src/views/EditorView/Layout/sidebar/tabs/sidebarEmptyStates.test.tsx
```

Expected: PASS with all model, adapter, component, and tab regressions.

- [ ] **Step 7: Run TypeScript validation**

Run:

```sh
pnpm typecheck
```

Expected: PASS. There must be no remaining import or use of `FlatSidebarRow`, `appendSectionBlock`, old `build*FlatRows` names, or core `kind: 'empty'` Sidebar rows.

- [ ] **Step 8: Confirm removed compatibility paths by search**

Run:

```sh
rg "FlatSidebarRow|appendSectionBlock|build(Graphs|Variables|Data|Charts|Nodes)FlatRows|kind:\s*['\"]empty['\"]" src/features/core/sidebar src/views/EditorView/Layout/sidebar
```

Expected: no matches. If an unrelated test fixture uses the word `empty`, narrow the assertion to Sidebar row construction and remove only the obsolete compatibility path.

---

### Task 4: Final Frontend Verification

**Files:**
- Verify only; modify files only if a failure is caused by Tasks 1–3.

**Interfaces:**
- Consumes: completed structured model, render adapter, empty-state components, tab integrations, and locale updates.
- Produces: fresh evidence that focused behavior, frontend types, broader tests, and whitespace checks pass.

- [ ] **Step 1: Run all focused Sidebar regressions again**

Run:

```sh
pnpm test -- src/features/core/sidebar/flatRows/sidebarPanelModel.test.ts src/views/EditorView/Layout/sidebar/sections/sidebarRenderRows.test.ts src/views/EditorView/Layout/sidebar/sections/sidebarEmptyStateComponents.test.tsx src/views/EditorView/Layout/sidebar/tabs/sidebarEmptyStates.test.tsx src/features/core/sidebar/sidebarStore.test.ts
```

Expected: PASS.

- [ ] **Step 2: Run frontend type checking**

Run:

```sh
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 3: Run the complete frontend test suite**

Run:

```sh
pnpm test
```

Expected: PASS. Do not modify unrelated failures; report them separately with their failing test names.

- [ ] **Step 4: Check formatting and whitespace damage**

Run:

```sh
git diff --check
```

Expected: no output and exit code 0.

- [ ] **Step 5: Review the final diff against acceptance criteria**

Run:

```sh
git --no-pager diff -- src/features/core/sidebar src/views/EditorView/Layout/sidebar src/app/i18n/locales/en-US.ts src/app/i18n/locales/zh-CN.ts
```

Confirm all of the following from the diff:

- Nodes unavailable state bypasses `SidebarFlatRowList` and `OverlayScrollbar`.
- Commands no-active-graph state uses `SidebarEmptyState`.
- Core Sidebar item rows contain no empty or section presentation variants.
- Section empty rendering exists only in the View-layer adapter/components.
- Graph, Variable, Data, and Chart resource handlers remain wired.
- Both locale files contain corresponding strings.
- `OverlayScrollbar.tsx` is unchanged.
