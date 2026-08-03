import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@/services/graph/graphService', () => ({
  GraphService: {
    unloadProjectGraph: vi.fn().mockResolvedValue(undefined),
  },
}));

vi.mock('@/services/nodeSystem/graphProjectionService', () => ({
  GraphProjectionService: {
    loadGraph: vi.fn(),
    hydrateGraph: vi.fn(),
  },
}));

vi.mock('@/features/core/dataStore/projectIOStore', () => ({
  invalidateGraphLoadOwnership: vi.fn(),
  useProjectIOStore: {
    getState: () => ({
      loadGraph: vi.fn(async () => true),
    }),
  },
}));

import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { resetEditorTabStore, seedEditorGroupTabs } from '@/features/core/layout/editorTabTestUtils';
import { useGraphDataStore } from '@/features/core/dataStore';
import {
  buildGraphResourceMeta,
  getDocumentState,
  markResourceLoaded,
  useDocumentStateStore,
  useResourceStore,
} from '@/features/core/resource';
import { isGraphCachedInMemory } from '@/features/core/dataStore/graphDocumentLoadPolicy';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { startProjectLifecycle } from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import {
  hydrateGraphProjection,
  resetGraphProjectionCoordinator,
} from '@/features/application/editorProjection/graphProjectionCoordinator';
import { GraphProjectionService } from '@/services/nodeSystem/graphProjectionService';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { suspendEditorGroupGraphSession } from './graphSessionLifecycle';
import { unloadGraphDocument } from './graphDocumentUnload';
import { activateEditorGroup } from './switchEditorTab';
import { activateGraphTab } from './activateGraphTab';

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

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
    useResourceStore.getState().clear();
    useDocumentStateStore.getState().clear();
    resetGraphProjectionCoordinator();
    startProjectLifecycle('project-instance-1');
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

  it('keeps a graph unloaded when a pending locale hydration resolves later', async () => {
    const graphPath = 'events/closed.yssbi-event';
    const current = makeEditorProjectionFixture({ graphPath, sourceRevision: 4, title: 'Current' });
    const localized = makeEditorProjectionFixture({ graphPath, sourceRevision: 4, title: 'Localized' });
    const pending = deferred<typeof localized.projection>();
    useGraphDataStore.getState().replaceProjection(graphPath, current.projection, 1);
    useResourceStore.getState().setSnapshot({
      resources: [buildGraphResourceMeta('event', graphPath, 'Closed')],
    });
    markResourceLoaded({ id: graphPath, kind: 'event' });
    vi.mocked(GraphProjectionService.hydrateGraph).mockReturnValue(pending.promise);

    const hydration = hydrateGraphProjection(graphPath, 'en-US');
    await unloadGraphDocument(graphPath);
    pending.resolve(localized.projection);
    await hydration;

    expect(useGraphDataStore.getState().hasGraph(graphPath)).toBe(false);
    expect(getDocumentState({ id: graphPath, kind: 'event' })?.loaded).toBe(false);
    expect(isGraphCachedInMemory(graphPath)).toBe(false);
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
