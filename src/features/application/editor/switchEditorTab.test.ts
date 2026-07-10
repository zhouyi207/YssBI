import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { activateEditorGroup } from './switchEditorTab';
import { activateGraphTab } from './activateGraphTab';

vi.mock('./activateGraphTab', () => ({
  activateGraphTab: vi.fn().mockResolvedValue(true),
}));

vi.mock('@/features/core/dataStore', () => ({
  getGraphByPath: vi.fn(),
}));

vi.mock('@/features/core/viewport', () => ({
  ensureGraphViewport: vi.fn(),
}));

vi.mock('@/features/core/editor/detail/variablesGraphScope', () => ({
  syncVariablesGraphScopeFromActiveTab: vi.fn(),
}));

describe('editor group activation', () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
          data: { component: 'GraphEditor', tabs: [] },
        },
        'group-b': {
          id: 'group-b',
          type: 'component',
          parentId: 'root',
          data: {
            component: 'GraphEditor',
            tabs: [{ id: 'events/B.yssbi-event', component: 'GraphEditor', type: 'event' }],
            activeTabId: 'events/B.yssbi-event',
          },
        },
      },
    });
  });

  it('activates the group and its current graph session together', async () => {
    await activateEditorGroup('group-b');

    expect(useLayoutStore.getState().activeEditorGroupId).toBe('group-b');
    expect(activateGraphTab).toHaveBeenCalledWith('events/B.yssbi-event', 'group-b');
  });
});
