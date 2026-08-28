import { beforeEach, describe, expect, it, vi } from 'vitest';
import { activateGraphTab, deactivateGraphTab } from './activateGraphTab';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useProjectIOStore } from '@/features/application/project/projectIOStore';
import { getDocumentState, markResourceLoaded } from '@/features/core/resource';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';
import { useDocumentStateStore } from '@/features/core/resource/documentStateStore';
import { unloadGraphDocument } from './graphDocumentUnload';

vi.mock('./graphDocumentUnload', () => ({
  unloadGraphDocument: vi.fn(async () => undefined),
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

describe('activateGraphTab', () => {
  const graphPath = 'events/Main.yssbi-event';

  beforeEach(() => {
    useGraphSessionStore.getState().reset();
    useGraphDataStore.setState({ graphEntities: {} });
    useDocumentStateStore.getState().clear();
    vi.restoreAllMocks();
    vi.mocked(unloadGraphDocument).mockResolvedValue(undefined);
  });

  it('calls loadGraph once and completes editor activation when graph is available', async () => {
    const fixture = makeEditorProjectionFixture({ graphPath });
    useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);
    markResourceLoaded({ id: graphPath, kind: 'event' });

    const loadGraph = vi.fn(async () => true);
    useProjectIOStore.setState({ loadGraph });

    const ok = await activateGraphTab(graphPath, 'editor-1');

    expect(ok).toBe(true);
    expect(loadGraph).toHaveBeenCalledTimes(1);
    expect(loadGraph).toHaveBeenCalledWith(graphPath);
    expect(useGraphSessionStore.getState().getFocusedGraphPath()).toBe(graphPath);
  });

  it('does not mark an empty graph cache loaded when loadGraph reports success', async () => {
    const loadGraph = vi.fn(async () => true);
    useProjectIOStore.setState({ loadGraph });

    const ok = await activateGraphTab(graphPath, 'editor-1');

    expect(ok).toBe(false);
    expect(useGraphSessionStore.getState().getFocusedGraphPath()).toBeNull();
    expect(getDocumentState({ id: graphPath, kind: 'event' })?.loaded).not.toBe(true);
  });

  it('loads the new graph before unloading the previous session', async () => {
    const previousPath = 'events/Previous.yssbi-event';
    const fixture = makeEditorProjectionFixture({ graphPath });
    useGraphDataStore.getState().replaceProjection(graphPath, fixture.projection, 1);
    useGraphSessionStore.getState().setFocusedSession('editor-1', previousPath);
    const order: string[] = [];
    vi.mocked(unloadGraphDocument).mockImplementation(async () => {
      order.push('unload');
    });
    useProjectIOStore.setState({
      loadGraph: vi.fn(async () => {
        order.push('load');
        return true;
      }),
    });

    await expect(activateGraphTab(graphPath, 'editor-1')).resolves.toBe(true);

    await vi.waitFor(() => expect(unloadGraphDocument).toHaveBeenCalledWith(previousPath));
    expect(order).toEqual(['load', 'unload']);
  });

  it('does not let an older failed activation overwrite a newer focused session', async () => {
    const pathB = 'events/B.yssbi-event';
    const pathC = 'events/C.yssbi-event';
    const pendingB = deferred<boolean>();
    const fixtureC = makeEditorProjectionFixture({ graphPath: pathC });
    useGraphDataStore.getState().replaceProjection(pathC, fixtureC.projection, 1);
    useGraphSessionStore.getState().setFocusedSession('editor-1', 'events/A.yssbi-event');
    useProjectIOStore.setState({
      loadGraph: vi.fn((path: string) => path === pathB ? pendingB.promise : Promise.resolve(true)),
    });

    const activationB = activateGraphTab(pathB, 'editor-1');
    await expect(activateGraphTab(pathC, 'editor-1')).resolves.toBe(true);
    pendingB.resolve(false);
    await expect(activationB).resolves.toBe(false);

    expect(useGraphSessionStore.getState().getFocusedGraphPath()).toBe(pathC);
  });

  it('rolls back session without unloading the previous graph when loadGraph fails', async () => {
    const loadGraph = vi.fn(async () => false);
    useProjectIOStore.setState({ loadGraph });

    const ok = await activateGraphTab(graphPath, 'editor-1');

    expect(ok).toBe(false);
    expect(loadGraph).toHaveBeenCalledTimes(1);
    expect(useGraphSessionStore.getState().getFocusedGraphPath()).toBeNull();
  });
});

describe('deactivateGraphTab', () => {
  beforeEach(() => {
    useGraphSessionStore.getState().reset();
  });

  it('clears session when the closed tab owned the focused graph', () => {
    useGraphSessionStore.getState().setFocusedSession('editor', 'g1');

    deactivateGraphTab('editor', 'g1');

    expect(useGraphSessionStore.getState().getFocusedGraphPath()).toBeNull();
  });

  it('keeps session when a background tab is closed', () => {
    useGraphSessionStore.getState().setFocusedSession('editor', 'g1');

    deactivateGraphTab('editor', 'g2');

    expect(useGraphSessionStore.getState().getFocusedGraphPath()).toBe('g1');
  });
});
