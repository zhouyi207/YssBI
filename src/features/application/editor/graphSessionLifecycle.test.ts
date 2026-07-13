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
import { resetEditorTabStore, seedEditorGroupTabs } from '@/features/core/layout/editorTabTestUtils';
import { useGraphDataStore } from '@/features/core/dataStore';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { suspendEditorGroupGraphSession } from './graphSessionLifecycle';
import { unloadGraphDocument } from './graphDocumentUnload';
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
    seedEditorGroupTabs('group-a', [
      { id: 'events/A.yssbi-event', component: 'GraphEditor', type: 'event' },
    ]);
    seedEditorGroupTabs('group-b', [
      { id: 'events/B.yssbi-event', component: 'GraphEditor', type: 'event' },
    ]);
  });

  it('keeps graph documents when their tab remains open in a suspended group', async () => {
    useGraphDataStore.setState({
      graphEntities: { 'events/B.yssbi-event': {} as never },
    });
    useGraphSessionStore.getState().setFocusedSession('group-b', 'events/B.yssbi-event');

    await suspendEditorGroupGraphSession('group-b');

    expect(useGraphDataStore.getState().graphEntities['events/B.yssbi-event']).toBeDefined();
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

  it('does not unload a graph that is still open in a tab when session is unbound', async () => {
    useGraphDataStore.setState({
      graphEntities: { 'events/A.yssbi-event': {} as never },
    });

    await unloadGraphDocument('events/A.yssbi-event');

    expect(useGraphDataStore.getState().graphEntities['events/A.yssbi-event']).toBeDefined();
  });

  it('unloads graphs that are no longer open in any tab', async () => {
    useGraphDataStore.setState({
      graphEntities: { 'events/closed.yssbi-event': {} as never },
    });

    await unloadGraphDocument('events/closed.yssbi-event');

    expect(useGraphDataStore.getState().graphEntities['events/closed.yssbi-event']).toBeUndefined();
  });

  it('activateEditorGroup keeps previous group graphs when their tabs remain open', async () => {
    useGraphDataStore.setState({
      graphEntities: {
        'events/A.yssbi-event': {} as never,
        'events/B.yssbi-event': {} as never,
      },
    });
    useGraphSessionStore.getState().setFocusedSession('group-a', 'events/A.yssbi-event');

    await activateEditorGroup('group-b');

    expect(useLayoutStore.getState().activeEditorGroupId).toBe('group-b');
    expect(useGraphDataStore.getState().graphEntities['events/A.yssbi-event']).toBeDefined();
    expect(activateGraphTab).toHaveBeenCalledWith('events/B.yssbi-event', 'group-b');
  });
});
