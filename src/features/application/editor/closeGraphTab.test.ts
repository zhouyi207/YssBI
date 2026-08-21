import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  getDocumentState,
  markResourceDirty,
  markResourceLoaded,
  useDocumentStateStore,
} from '@/features/core/resource';
import { closeGraphTab } from './closeGraphTab';

const dockview = vi.hoisted(() => {
  const panels: Array<{
    panelInstanceId: string;
    groupId: string;
    active: boolean;
    tab: {
      resourceRef: string;
      kind: 'event';
      data: { layoutTab: Record<string, unknown> };
    };
  }> = [];

  return {
    panels,
    findPanelsByResource: vi.fn((resourceRef: string) =>
      panels.filter((panel) => panel.tab.resourceRef === resourceRef)),
    listPanels: vi.fn(() => panels),
    getActivePanel: vi.fn(() => panels.find((panel) => panel.active)),
    remove: vi.fn(async (panelInstanceId: string) => {
      const index = panels.findIndex((panel) => panel.panelInstanceId === panelInstanceId);
      if (index < 0) return false;
      panels.splice(index, 1);
      return true;
    }),
  };
});

vi.mock('@/features/core/dockview', () => ({
  editorDockviewPort: dockview,
}));

vi.mock('./graphDocumentUnload', () => ({
  unloadGraphDocument: vi.fn(async () => undefined),
}));

vi.mock('./switchEditorTab', () => ({
  switchEditorTab: vi.fn(async () => true),
}));

vi.mock('./activateGraphTab', () => ({
  deactivateGraphTab: vi.fn(),
}));

vi.mock('@/features/core/editor/detail/clearDetailFocusForClosedTab', () => ({
  clearDetailFocusForClosedTab: vi.fn(),
}));

vi.mock('./rightSidebarActions', () => ({
  focusDetails: vi.fn(),
}));

vi.mock('@/features/core/editor/detail/variablesGraphScope', () => ({
  syncVariablesGraphScopeAfterClose: vi.fn(),
}));

vi.mock('@/features/core/editor/stores/useEditorStore', () => ({
  useEditorStore: {
    getState: () => ({ detailFocus: { kind: 'event', path: 'events/Shared.yssbi-event' } }),
  },
}));

vi.mock('@/features/core/viewport', () => ({
  releaseEditorViewport: vi.fn(),
}));

vi.mock('@/features/core/viewport/viewportScope', () => ({
  editorViewportScope: (groupId: string, graphPath: string) => ({ groupId, graphPath }),
}));

vi.mock('@/utils/appLogger', () => ({
  logger: { graph: { warn: vi.fn() } },
}));

const graphPath = 'events/Shared.yssbi-event';
const graphRef = { id: graphPath, kind: 'event' } as const;

function graphPanel(panelInstanceId: string, groupId: string, active: boolean) {
  return {
    panelInstanceId,
    groupId,
    active,
    tab: {
      resourceRef: graphPath,
      kind: 'event' as const,
      data: {
        layoutTab: {
          id: graphPath,
          type: 'event',
          component: 'GraphEditor',
        },
      },
    },
  };
}

describe('closeGraphTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    dockview.panels.splice(
      0,
      dockview.panels.length,
      graphPanel('panel-a', 'group-a', false),
      graphPanel('panel-b', 'group-b', true),
    );
    useDocumentStateStore.getState().clear();
    markResourceLoaded(graphRef);
    markResourceDirty(graphRef, true);
  });

  it('keeps dirty loaded document state when another panel shows the same graph', async () => {
    await expect(closeGraphTab(graphPath, 'group-a', true)).resolves.toBe(true);

    expect(dockview.findPanelsByResource(graphPath).map((panel) => panel.panelInstanceId))
      .toEqual(['panel-b']);
    expect(getDocumentState(graphRef)).toMatchObject({ dirty: true, loaded: true });
  });
});
