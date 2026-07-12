import { describe, expect, it, vi, beforeEach } from 'vitest';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { openEditorTab } from './openEditorTab';

describe('openEditorTab', () => {
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

    expect(moveTab).toHaveBeenCalledWith('sidebar', 'ws1', 'default_editor');
    expect(useLayoutStore.getState().activeEditorGroupId).toBe('default_editor');
  });
});
