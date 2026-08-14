import { beforeEach, describe, expect, it } from 'vitest';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { openEditorTab } from './openEditorTab';

describe('openEditorTab insertIndex', () => {
  beforeEach(() => {
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useLayoutStore.setState({
      nodes: {
        default_editor: {
          id: 'default_editor',
          type: 'component',
          parentId: 'center',
          data: { component: 'GraphEditor' },
        },
      },
      activeEditorGroupId: 'default_editor',
    } as Partial<ReturnType<typeof useLayoutStore.getState>>);
    useEditorTabStore.getState().initGroupPlacement('default_editor', [
      { id: 'g1', component: 'GraphEditor', type: 'event', pinned: true },
      { id: 'g2', component: 'GraphEditor', type: 'event', pinned: true },
    ], 'g1');
  });

  it('inserts a new tab at the requested index', () => {
    openEditorTab(
      { id: 'g3', component: 'GraphEditor', type: 'function', pinned: true },
      { insertIndex: 1, pinned: true },
    );

    const placement = useEditorTabStore.getState().getPlacement('default_editor');
    expect(placement.tabIds).toEqual(['g1', 'g3', 'g2']);
    expect(placement.activeTabId).toBe('g3');
  });

  it('reorders and activates an existing tab while clearing stale graph selection', () => {
    useEditorTabStore.getState().setSelectedConnectionIds('default_editor', ['edge-from-g1']);

    openEditorTab(
      { id: 'g2', component: 'GraphEditor', type: 'event', pinned: true },
      { targetGroupId: 'default_editor', insertIndex: 0, pinned: true },
    );

    const placement = useEditorTabStore.getState().getPlacement('default_editor');
    expect(placement.tabIds).toEqual(['g2', 'g1']);
    expect(placement.activeTabId).toBe('g2');
    expect(placement.selectedNodeIds).toEqual([]);
    expect(placement.selectedConnectionIds).toEqual([]);
  });
});

describe('openEditorTab chrome recovery', () => {
  beforeEach(() => {
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useLayoutStore.setState({
      nodes: {
        sidebar: {
          id: 'sidebar',
          type: 'component',
          parentId: 'root',
          data: {
            component: 'Sidebar',
            isFixed: true,
            tabs: [{ id: 'ws1', component: 'WorksheetEditor', type: 'worksheet' }],
            activeTabId: 'ws1',
          },
        },
        default_editor: {
          id: 'default_editor',
          type: 'component',
          parentId: 'center',
          data: { component: 'GraphEditor' },
        },
      },
      activeEditorGroupId: null,
    } as Partial<ReturnType<typeof useLayoutStore.getState>>);
    useEditorTabStore.getState().ensureGroupPlacement('default_editor');
  });

  it('moves an existing worksheet tab from sidebar chrome into the editor group', () => {
    openEditorTab({
      id: 'ws1',
      component: 'WorksheetEditor',
      type: 'worksheet',
    });

    const placement = useEditorTabStore.getState().getPlacement('default_editor');
    expect(placement.tabIds).toEqual(['ws1']);
    expect(placement.activeTabId).toBe('ws1');
    expect(useLayoutStore.getState().activeEditorGroupId).toBe('default_editor');
  });
});
