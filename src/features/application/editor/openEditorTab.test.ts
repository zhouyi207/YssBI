import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { openEditorTab } from './openEditorTab';

describe('openEditorTab insertIndex', () => {
  beforeEach(() => {
    useLayoutStore.setState({
      nodes: {
        default_editor: {
          id: 'default_editor',
          type: 'component',
          parentId: 'center',
          data: {
            component: 'GraphEditor',
            tabs: [
              { id: 'g1', component: 'GraphEditor', type: 'event', pinned: true },
              { id: 'g2', component: 'GraphEditor', type: 'event', pinned: true },
            ],
            activeTabId: 'g1',
          },
        },
      },
      activeEditorGroupId: 'default_editor',
    } as Partial<ReturnType<typeof useLayoutStore.getState>>);
  });

  it('inserts a new tab at the requested index', () => {
    openEditorTab(
      { id: 'g3', component: 'GraphEditor', type: 'function', pinned: true },
      { insertIndex: 1, pinned: true },
    );

    const tabs = useLayoutStore.getState().nodes.default_editor?.data?.tabs ?? [];
    expect(tabs.map((tab) => tab.id)).toEqual(['g1', 'g3', 'g2']);
    expect(useLayoutStore.getState().nodes.default_editor?.data?.activeTabId).toBe('g3');
  });

  it('reorders an existing tab within the same editor group', () => {
    const moveTab = vi.spyOn(useLayoutStore.getState(), 'moveTab');

    openEditorTab(
      { id: 'g2', component: 'GraphEditor', type: 'event', pinned: true },
      { targetGroupId: 'default_editor', insertIndex: 0, pinned: true },
    );

    expect(moveTab).toHaveBeenCalledWith('default_editor', 'g2', 'default_editor', 0);
  });
});

describe('openEditorTab chrome recovery', () => {
  beforeEach(() => {
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
          data: { component: 'GraphEditor', tabs: [], activeTabId: undefined },
        },
      },
      activeEditorGroupId: null,
    } as Partial<ReturnType<typeof useLayoutStore.getState>>);
  });

  it('moves an existing worksheet tab from sidebar chrome into the editor group', () => {
    const moveTab = vi.spyOn(useLayoutStore.getState(), 'moveTab');

    openEditorTab({
      id: 'ws1',
      component: 'WorksheetEditor',
      type: 'worksheet',
    });

    expect(moveTab).toHaveBeenCalledWith('sidebar', 'ws1', 'default_editor', undefined);
    expect(useLayoutStore.getState().activeEditorGroupId).toBe('default_editor');
  });
});
