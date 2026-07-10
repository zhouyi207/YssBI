import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@/services/graph/graphService', () => ({
  GraphService: {
    unloadProjectGraph: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('@/features/core/dataStore/projectIOStore', () => ({
  useProjectIOStore: {
    getState: () => ({
      loadGraph: vi.fn(async () => true),
    }),
  },
}));

import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useGraphDataStore } from '@/features/core/dataStore';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { suspendEditorGroupGraphSession, unloadGraphDocument } from './graphSessionLifecycle';
import { activateEditorGroup } from './switchEditorTab';
import { activateGraphTab } from './activateGraphTab';

vi.mock('./activateGraphTab', async (importOriginal) => {
  const actual = await importOriginal<typeof import('./activateGraphTab')>();
  return {
    ...actual,
    activateGraphTab: vi.fn(actual.activateGraphTab),
  };
});

describe('graphSessionLifecycle', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useGraphSessionStore.getState().reset();
    useGraphDataStore.setState({ graphEntities: {} });
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
          data: {
            component: 'GraphEditor',
            tabs: [{ id: 'events/A.yssbi-event', component: 'GraphEditor', type: 'event' }],
            activeTabId: 'events/A.yssbi-event',
          },
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

  it('unloads graph documents even when the tab remains open in an inactive group', async () => {
    useGraphDataStore.setState({
      graphEntities: { 'events/B.yssbi-event': {} as never },
    });
    useGraphSessionStore.getState().setFocusedSession('group-b', 'events/B.yssbi-event');

    await suspendEditorGroupGraphSession('group-b');

    expect(useGraphDataStore.getState().graphEntities['events/B.yssbi-event']).toBeUndefined();
    expect(useGraphSessionStore.getState().getFocusedGraphPath()).toBeNull();
  });

  it('does not unload the focused graph session', async () => {
    useGraphDataStore.setState({
      graphEntities: { 'events/A.yssbi-event': {} as never },
    });
    useGraphSessionStore.getState().setFocusedSession('group-a', 'events/A.yssbi-event');

    await unloadGraphDocument('events/A.yssbi-event');

    expect(useGraphDataStore.getState().graphEntities['events/A.yssbi-event']).toBeDefined();
  });

  it('activateEditorGroup suspends the previous group before hydrating the next', async () => {
    useGraphDataStore.setState({
      graphEntities: {
        'events/A.yssbi-event': {} as never,
        'events/B.yssbi-event': {} as never,
      },
    });
    useGraphSessionStore.getState().setFocusedSession('group-a', 'events/A.yssbi-event');

    await activateEditorGroup('group-b');

    expect(useLayoutStore.getState().activeEditorGroupId).toBe('group-b');
    expect(useGraphDataStore.getState().graphEntities['events/A.yssbi-event']).toBeUndefined();
    expect(activateGraphTab).toHaveBeenCalledWith('events/B.yssbi-event', 'group-b');
  });
});
