import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { resetEditorTabStore, seedEditorGroupTabs } from '@/features/core/layout/editorTabTestUtils';
import { activateEditorGroup, focusEditorGroupSync } from './switchEditorTab';
import { activateGraphTab } from './activateGraphTab';

vi.mock('./activateGraphTab', () => ({
  activateGraphTab: vi.fn().mockResolvedValue(true),
}));

vi.mock('@/features/core/editor/detail/variablesGraphScope', () => ({
  syncVariablesGraphScopeFromActiveTab: vi.fn(),
}));

describe('editor group activation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetEditorTabStore();
    useLayoutStore.setState({
      rootId: 'root',
      activeEditorGroupId: 'group-a',
      nodes: {
        root: {
          id: 'root',
          type: 'row',
          parentId: null,
          children: ['group-a', 'group-b'],
        },
        'group-a': {
          id: 'group-a',
          type: 'component',
          parentId: 'root',
          data: { component: 'GraphEditor' },
        },
        'group-b': {
          id: 'group-b',
          type: 'component',
          parentId: 'root',
          data: { component: 'GraphEditor' },
        },
      },
    });
    seedEditorGroupTabs('group-b', [
      { id: 'events/B.yssbi-event', component: 'GraphEditor', type: 'event' },
    ]);
  });

  it('activates the group and its current graph session together', async () => {
    await activateEditorGroup('group-b');

    expect(useLayoutStore.getState().activeEditorGroupId).toBe('group-b');
    expect(activateGraphTab).toHaveBeenCalledWith('events/B.yssbi-event', 'group-b');
  });

  it('focusEditorGroupSync updates layout focus before graph hydrate', () => {
    focusEditorGroupSync('group-b');

    expect(useLayoutStore.getState().activeEditorGroupId).toBe('group-b');
    expect(activateGraphTab).not.toHaveBeenCalled();
  });
});
