import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useGraphDataStore, useProjectIOStore } from '@/features/core/dataStore';
import { useEditorStore } from '@/features/core/editor';
import { resolveDetailTarget } from '@/features/core/editor/detail/resolveDetailTarget';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import {
  buildGraphResourceMeta,
  isGraphResourceDirty,
  markResourceDirty,
  useDocumentStateStore,
  useResourceStore,
} from '@/features/core/resource';
import { uiStore } from '@/features/core/ui/UIStore';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { GraphService } from '@/services/graph/graphService';
import { closeGraphTab } from './closeGraphTab';

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => { resolve = settle; });
  return { promise, resolve };
}

function installGraphRevision(graphPath: string, graphRevision: number): void {
  useGraphDataStore.setState({
    graphEntities: {
      [graphPath]: {
        basis: {
                  graphPath,
                  graphRevision,
                  registryFingerprint: '0000000000000000000000000000000000000000000000000000000000000000',
                  resourceVersions: {},
                },
        sourceRevision: graphRevision,
        requestGeneration: 1,
        diagnostics: [],
        outcome: { type: 'success' },
        hasBlockingDiagnostics: false,
        nodes: {},
        pins: {},
        connections: {},
        graphNodes: [],
        nodePins: {},
        pinConnections: {},
      },
    },
  });
}

describe('closeGraphTab', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useLayoutStore.setState({
      rootId: 'root',
      nodes: {
        root: {
          id: 'root',
          type: 'row',
          parentId: null,
          children: ['editor'],
        },
        editor: {
          id: 'editor',
          type: 'component',
          parentId: 'root',
          data: { component: 'GraphEditor' },
        },
      },
      activeEditorGroupId: 'editor',
    });
    useEditorTabStore.getState().initGroupPlacement('editor', [
      { id: 'g1', component: 'GraphEditor', type: 'event' },
      { id: 'g2', component: 'GraphEditor', type: 'event' },
    ], 'g1');
    useEditorTabStore.getState().setSelectedNodeIds('editor', ['node-from-g1']);
    useGraphDataStore.setState({ graphEntities: {} });
    useResourceStore.getState().clear();
    useDocumentStateStore.getState().clear();
    useProjectIOStore.setState({ projectInstanceId: 'project-a' });
    projectPublicationCoordinator.startProject('project-a', 0);
    useEditorStore.getState().clearDetailFocus();
    useGraphSessionStore.getState().reset();
    vi.spyOn(GraphService, 'unloadProjectGraph').mockResolvedValue();
  });

  it('loads and selects the remaining active graph after closing a tab', async () => {
    const loadGraph = vi.fn(async () => true);
    useProjectIOStore.setState({ loadGraph });

    useEditorStore.getState().setDetailFocus({ kind: 'variable', id: 'var-1' });

    const closed = await closeGraphTab('g1', 'editor', true);

    const placement = useEditorTabStore.getState().getPlacement('editor');
    expect(closed).toBe(true);
    expect(placement.activeTabId).toBe('g2');
    expect(placement.selectedNodeIds).toEqual([]);
    expect(loadGraph).toHaveBeenCalledWith('g2');

    const detailTarget = resolveDetailTarget({
      detailFocus: useEditorStore.getState().detailFocus,
      selectedLog: null,
    });
    expect(detailTarget).toEqual({ kind: 'variable', id: 'var-1' });
  });

  it('moves detail focus to the remaining active graph when the closed tab was focused', async () => {
    useEditorStore.getState().setDetailFocus({ kind: 'event', path: 'g1' });

    await closeGraphTab('g1', 'editor', true);

    expect(useEditorStore.getState().detailFocus).toEqual({ kind: 'event', path: 'g2' });
  });

  it('ignores a delayed save completion after the project lifecycle changes', async () => {
    installGraphRevision('g1', 1);
    useResourceStore.getState().upsertResource(buildGraphResourceMeta('event', 'g1', 'g1'));
    markResourceDirty({ id: 'g1', kind: 'event' }, true);
    vi.spyOn(uiStore, 'confirm').mockResolvedValue(true);
    const save = deferred<Awaited<ReturnType<typeof GraphService.saveProjectGraph>>>();
    vi.spyOn(GraphService, 'saveProjectGraph').mockReturnValue(save.promise);

    const completion = closeGraphTab('g1', 'editor');
    await vi.waitFor(() => expect(GraphService.saveProjectGraph).toHaveBeenCalled());
    useProjectIOStore.setState({ projectInstanceId: 'project-b' });
    projectPublicationCoordinator.startProject('project-b', 0);
    save.resolve({} as Awaited<ReturnType<typeof GraphService.saveProjectGraph>>);

    await expect(completion).resolves.toBe(false);
    expect(useEditorTabStore.getState().getPlacement('editor').tabIds).toContain('g1');
    expect(isGraphResourceDirty('g1', 'event')).toBe(true);
  });

  it('keeps the graph dirty when its revision advances while save is pending', async () => {
    installGraphRevision('g1', 1);
    useResourceStore.getState().upsertResource(buildGraphResourceMeta('event', 'g1', 'g1'));
    markResourceDirty({ id: 'g1', kind: 'event' }, true);
    vi.spyOn(uiStore, 'confirm').mockResolvedValue(true);
    const save = deferred<Awaited<ReturnType<typeof GraphService.saveProjectGraph>>>();
    vi.spyOn(GraphService, 'saveProjectGraph').mockReturnValue(save.promise);

    const completion = closeGraphTab('g1', 'editor');
    await vi.waitFor(() => expect(GraphService.saveProjectGraph).toHaveBeenCalled());
    installGraphRevision('g1', 2);
    save.resolve({} as Awaited<ReturnType<typeof GraphService.saveProjectGraph>>);

    await expect(completion).resolves.toBe(false);
    expect(useEditorTabStore.getState().getPlacement('editor').tabIds).toContain('g1');
    expect(isGraphResourceDirty('g1', 'event')).toBe(true);
  });

  it('preserves focused session when closing a background tab', async () => {
    useGraphSessionStore.getState().setFocusedSession('editor', 'g1');
    vi.spyOn(useProjectIOStore.getState(), 'loadGraph').mockResolvedValue(true);

    await closeGraphTab('g2', 'editor', true);

    expect(useGraphSessionStore.getState().getFocusedGraphPath()).toBe('g1');
  });
});
