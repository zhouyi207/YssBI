import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import {
  activateGraphTab,
  deactivateGraphTab,
} from '@/features/application/editor/activateGraphTab';
import { useGraphSessionStore } from './graphSessionStore';

describe('graphSessionStore', () => {
  beforeEach(() => {
    useGraphSessionStore.getState().reset();
  });

  it('tracks active graph path by group', () => {
    const store = useGraphSessionStore.getState();
    expect(store.setGroupActivePath('editor-a', 'events/A.yssbi-event')).toBeNull();
    expect(store.setGroupActivePath('editor-a', 'events/B.yssbi-event')).toBe('events/A.yssbi-event');
    expect(store.getGroupActivePath('editor-a')).toBe('events/B.yssbi-event');
    expect(store.isPathActiveInAnyGroup('events/B.yssbi-event')).toBe(true);
    expect(store.isPathActiveInAnyGroup('events/A.yssbi-event')).toBe(false);
  });

  it('activateGraphTab forces backend hydration and deactivates previous graph in group', async () => {
    useLayoutStore.setState({
      nodes: {
        root: { id: 'root', type: 'row', parentId: null, children: ['editor-a'] },
        'editor-a': {
          id: 'editor-a',
          type: 'component',
          parentId: 'root',
          data: { component: 'GraphEditor', tabs: [], activeTabId: undefined },
        },
      },
      activeEditorGroupId: 'editor-a',
    });

    const loadGraph = vi.fn(async () => true);
    useProjectIOStore.setState({ loadGraph });
    useGraphSessionStore.getState().setGroupActivePath('editor-a', 'events/Old.yssbi-event');

    const activated = await activateGraphTab('events/New.yssbi-event', 'editor-a');

    expect(activated).toBe(true);
    expect(loadGraph).toHaveBeenCalledWith('events/New.yssbi-event');
    expect(useGraphSessionStore.getState().getGroupActivePath('editor-a')).toBe('events/New.yssbi-event');

    deactivateGraphTab('editor-a');
    expect(useGraphSessionStore.getState().getGroupActivePath('editor-a')).toBeNull();
  });
});
