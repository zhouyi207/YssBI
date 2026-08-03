// @vitest-environment happy-dom
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useProjectionLocaleSync } from './useProjectionLocaleSync';
import { resetGraphProjectionCoordinator } from '@/features/application/editorProjection/graphProjectionCoordinator';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import {
  buildGraphResourceMeta,
  markResourceLoaded,
  useDocumentStateStore,
  useResourceStore,
} from '@/features/core/resource';
import { editorViewportScope, useViewportStore } from '@/features/core/viewport';
import { viewportScopeKey } from '@/features/core/viewport/viewportScope';
import { GraphProjectionService } from '@/services/nodeSystem/graphProjectionService';
import { makeEditorProjectionFixture } from '@/tests/helpers/editorProjectionFixtures';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const localeState = vi.hoisted(() => ({ language: 'zh-CN' }));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: {
      language: localeState.language,
      resolvedLanguage: localeState.language,
    },
  }),
}));

vi.mock('@/services/nodeSystem/graphProjectionService', () => ({
  GraphProjectionService: {
    loadGraph: vi.fn(),
    hydrateGraph: vi.fn(),
  },
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function Harness() {
  useProjectionLocaleSync();
  return null;
}

describe('useProjectionLocaleSync', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    localeState.language = 'zh-CN';
    resetGraphProjectionCoordinator();
    clearProjectLifecycle();
    startProjectLifecycle('project-instance-1');
    useGraphDataStore.setState({ graphEntities: {} });
    useResourceStore.getState().clear();
    useDocumentStateStore.getState().clear();
    useViewportStore.getState().clear();
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    clearProjectLifecycle();
    host.remove();
  });

  it('rehydrates each loaded graph once and preserves canvas viewport state', async () => {
    const eventPath = 'events/Main.yssbi-event';
    const functionPath = 'functions/Compute.yssbi-function';
    const unloadedPath = 'events/Closed.yssbi-event';
    const currentEvent = makeEditorProjectionFixture({
      graphPath: eventPath,
      sourceRevision: 4,
      title: '当前事件',
    });
    const currentFunction = makeEditorProjectionFixture({
      graphPath: functionPath,
      sourceRevision: 7,
      title: '当前函数',
    });
    const localizedEvent = makeEditorProjectionFixture({
      graphPath: eventPath,
      sourceRevision: 4,
      title: 'Localized event',
    });
    const localizedFunction = makeEditorProjectionFixture({
      graphPath: functionPath,
      sourceRevision: 7,
      title: 'Localized function',
    });
    useGraphDataStore.getState().replaceProjection(eventPath, currentEvent.projection, 1);
    useGraphDataStore.getState().replaceProjection(functionPath, currentFunction.projection, 1);
    useResourceStore.getState().setSnapshot({
      resources: [
        buildGraphResourceMeta('event', eventPath, 'Main'),
        buildGraphResourceMeta('function', functionPath, 'Compute'),
        buildGraphResourceMeta('event', unloadedPath, 'Closed'),
      ],
    });
    markResourceLoaded({ id: eventPath, kind: 'event' });
    markResourceLoaded({ id: functionPath, kind: 'function' });
    const viewportScope = editorViewportScope('default_editor', eventPath);
    useViewportStore.getState().setViewport(viewportScope, { x: 120, y: -30, scale: 1.5 });
    vi.mocked(GraphProjectionService.hydrateGraph).mockImplementation(async (
      projectInstanceId,
      graphPath,
      locale,
    ) => {
      expect(projectInstanceId).toBe('project-instance-1');
      expect(locale).toBe('en-US');
      return graphPath === eventPath ? localizedEvent.projection : localizedFunction.projection;
    });

    await act(async () => root.render(createElement(Harness)));
    expect(GraphProjectionService.hydrateGraph).not.toHaveBeenCalled();

    localeState.language = 'en-US';
    await act(async () => root.render(createElement(Harness)));
    await vi.waitFor(() => {
      expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledTimes(2);
    });

    expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledWith(
      'project-instance-1',
      eventPath,
      'en-US',
    );
    expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledWith(
      'project-instance-1',
      functionPath,
      'en-US',
    );
    expect(GraphProjectionService.hydrateGraph).not.toHaveBeenCalledWith(
      'project-instance-1',
      unloadedPath,
      'en-US',
    );
    expect(useGraphDataStore.getState().graphEntities[eventPath]).toMatchObject({
      sourceRevision: 4,
      nodes: { 'local-node': { title: 'Localized event' } },
    });
    expect(useGraphDataStore.getState().graphEntities[functionPath]).toMatchObject({
      sourceRevision: 7,
      nodes: { 'local-node': { title: 'Localized function' } },
    });
    expect(useViewportStore.getState().viewports[viewportScopeKey(viewportScope)])
      .toEqual({ x: 120, y: -30, scale: 1.5 });
  });

  it('ignores an older locale response after a newer language request starts', async () => {
    const graphPath = 'events/Main.yssbi-event';
    const current = makeEditorProjectionFixture({ graphPath, sourceRevision: 5, title: '当前' });
    const olderLocale = makeEditorProjectionFixture({ graphPath, sourceRevision: 5, title: 'English' });
    const latestLocale = makeEditorProjectionFixture({ graphPath, sourceRevision: 5, title: '中文' });
    const pendingEnglish = deferred<typeof olderLocale.projection>();
    useGraphDataStore.getState().replaceProjection(graphPath, current.projection, 1);
    useResourceStore.getState().setSnapshot({
      resources: [buildGraphResourceMeta('event', graphPath, 'Main')],
    });
    markResourceLoaded({ id: graphPath, kind: 'event' });
    vi.mocked(GraphProjectionService.hydrateGraph)
      .mockReturnValueOnce(pendingEnglish.promise)
      .mockResolvedValueOnce(latestLocale.projection);

    await act(async () => root.render(createElement(Harness)));
    localeState.language = 'en-US';
    await act(async () => root.render(createElement(Harness)));
    await vi.waitFor(() => {
      expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledTimes(1);
    });
    localeState.language = 'zh-CN';
    await act(async () => root.render(createElement(Harness)));
    await vi.waitFor(() => {
      expect(GraphProjectionService.hydrateGraph).toHaveBeenCalledTimes(2);
    });
    pendingEnglish.resolve(olderLocale.projection);
    await act(async () => {
      await pendingEnglish.promise;
      await Promise.resolve();
    });

    expect(GraphProjectionService.hydrateGraph).toHaveBeenNthCalledWith(
      1,
      'project-instance-1',
      graphPath,
      'en-US',
    );
    expect(GraphProjectionService.hydrateGraph).toHaveBeenNthCalledWith(
      2,
      'project-instance-1',
      graphPath,
      'zh-CN',
    );
    expect(useGraphDataStore.getState().graphEntities[graphPath]).toMatchObject({
      nodes: { 'local-node': { title: '中文' } },
      requestGeneration: 3,
    });
  });
});
