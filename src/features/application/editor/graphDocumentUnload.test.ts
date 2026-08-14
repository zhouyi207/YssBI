import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useGraphDataStore, useProjectIOStore } from '@/features/core/dataStore';
import {
  buildGraphResourceMeta,
  getDocumentState,
  markResourceLoaded,
  useDocumentStateStore,
  useResourceStore,
} from '@/features/core/resource';
import {
  beginGraphLoadLifecycle,
  loadGraphProjection,
  resetGraphProjectionCoordinator,
} from '@/features/application/editorProjection/graphProjectionCoordinator';
import { GraphProjectionService } from '@/services/nodeSystem/graphProjectionService';
import { GraphService } from '@/services/graph/graphService';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { unloadGraphDocument } from './graphDocumentUnload';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import {
  installCoreApplicationTestPorts,
  resetCoreApplicationTestPorts,
  type CoreApplicationTestPorts,
} from '@/features/application/testHelpers/coreApplicationPorts';

vi.mock('@/features/application/editor/graphDocumentRetention', () => ({
  shouldRetainGraphDocument: () => false,
}));

vi.mock('@/services/nodeSystem/graphProjectionService', () => ({
  GraphProjectionService: {
    loadGraph: vi.fn(),
    hydrateGraph: vi.fn(),
  },
}));

vi.mock('@/services/graph/graphService', () => ({
  GraphService: {
    unloadProjectGraph: vi.fn(),
  },
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

describe('graph document lifecycle ownership', () => {
  const graphPath = 'events/Main.yssbi-event';
  let ports: CoreApplicationTestPorts;

  beforeEach(() => {
    vi.clearAllMocks();
    resetGraphProjectionCoordinator();
    ports = installCoreApplicationTestPorts({
      projectIO: {
        beginGraphLoad: vi.fn(beginGraphLoadLifecycle),
        loadGraphProjection: vi.fn(loadGraphProjection),
      },
    });
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject('project-instance-1', 0);
    useGraphDataStore.setState({ graphEntities: {} });
    useProjectIOStore.setState({ projectInstanceId: 'project-instance-1' });
    useResourceStore.getState().clear();
    useDocumentStateStore.getState().clear();
    useResourceStore.getState().setSnapshot({
      resources: [buildGraphResourceMeta('event', graphPath, 'Main')],
      graphOrder: [graphPath],
    });
    vi.mocked(GraphService.unloadProjectGraph).mockResolvedValue();
  });

  afterEach(resetCoreApplicationTestPorts);

  it('starts a new load when an initial pending load is unloaded and immediately reopened', async () => {
    const oldFixture = makeEditorProjectionFixture({ graphPath, title: 'Old load' });
    const reopenedFixture = makeEditorProjectionFixture({ graphPath, title: 'Reopened load' });
    const oldLoad = deferred<typeof oldFixture.projection>();
    const reopenedLoad = deferred<typeof reopenedFixture.projection>();
    vi.mocked(GraphProjectionService.loadGraph)
      .mockReturnValueOnce(oldLoad.promise)
      .mockReturnValueOnce(reopenedLoad.promise);

    const initial = useProjectIOStore.getState().loadGraph(graphPath);
    await unloadGraphDocument(graphPath);
    const reopened = useProjectIOStore.getState().loadGraph(graphPath);

    expect(ports.projectIO.loadGraphProjection).toHaveBeenCalledTimes(2);
    expect(vi.mocked(ports.projectIO.loadGraphProjection).mock.calls[0]?.[1]).toBeLessThan(
      vi.mocked(ports.projectIO.loadGraphProjection).mock.calls[1]?.[1] ?? 0,
    );

    oldLoad.resolve(oldFixture.projection);
    await expect(initial).resolves.toBe(false);
    reopenedLoad.resolve(reopenedFixture.projection);
    await expect(reopened).resolves.toBe(true);

    expect(useGraphDataStore.getState().graphEntities[graphPath]?.nodes['local-node'].title)
      .toBe('Reopened load');
  });

  it('does not let an old unload completion overwrite a newer successful load', async () => {
    const current = makeEditorProjectionFixture({ graphPath, title: 'Current' });
    const reopened = makeEditorProjectionFixture({ graphPath, title: 'Reopened' });
    useGraphDataStore.getState().replaceProjection(graphPath, current.projection, 1);
    markResourceLoaded({ id: graphPath, kind: 'event' });
    const pendingUnload = deferred<void>();
    vi.mocked(GraphService.unloadProjectGraph).mockReturnValue(pendingUnload.promise);
    vi.mocked(GraphProjectionService.loadGraph).mockResolvedValue(reopened.projection);

    const unloading = unloadGraphDocument(graphPath);
    await expect(useProjectIOStore.getState().loadGraph(graphPath)).resolves.toBe(true);
    markResourceLoaded({ id: graphPath, kind: 'event' });
    expect(getDocumentState({ id: graphPath, kind: 'event' })?.loaded).toBe(true);

    pendingUnload.resolve();
    await unloading;

    expect(getDocumentState({ id: graphPath, kind: 'event' })?.loaded).toBe(true);
    expect(useGraphDataStore.getState().graphEntities[graphPath]?.nodes['local-node'].title)
      .toBe('Reopened');
    expect(vi.mocked(GraphService.unloadProjectGraph).mock.calls[0]?.[2])
      .toBe('project-instance-1');
    expect(vi.mocked(GraphService.unloadProjectGraph).mock.calls[0]?.[1]).toBeLessThan(
      vi.mocked(ports.projectIO.loadGraphProjection).mock.calls[0]?.[1] ?? 0,
    );
  });
});
