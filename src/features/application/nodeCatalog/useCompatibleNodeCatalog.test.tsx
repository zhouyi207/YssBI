// @vitest-environment happy-dom
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { CatalogService, type LocalizedCatalogDto } from '@/services/nodeSystem/catalogService';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import { useCompatibleNodeCatalog } from './useCompatibleNodeCatalog';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const localeState = vi.hoisted(() => ({ language: 'en-US' }));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    i18n: {
      language: localeState.language,
      resolvedLanguage: localeState.language,
    },
  }),
}));

vi.mock('@/services/nodeSystem/catalogService', () => ({
  CatalogService: { getCompatibleNodeCatalog: vi.fn() },
}));

const sourcePort: PortAddressDto = {
  kind: 'declared',
  nodeId: '00000000-0000-0000-0000-000000000101',
  portKey: 'value',
};

function catalog(projectInstanceId: string, itemId: string): LocalizedCatalogDto {
  return {
    projectInstanceId,
    registryFingerprint: '0'.repeat(64),
    resourcePublicationRevision: 7,
    locale: 'en-US',
    categories: [{ categoryId: 'compatible', title: 'Compatible', searchText: 'compatible' }],
    items: [{
      nodeTypeId: itemId,
      title: itemId,
      description: null,
      documentation: null,
      categoryId: 'compatible',
      iconId: 'test',
      styleId: 'default',
      aliases: [],
      technicalTerms: [],
      backendSearchText: [itemId],
      resourceNames: [],
      ports: [],
      parameters: [],
      creation: { kind: 'static', nodeTypeId: itemId },
    }],
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function Harness({ graphRevision = 4 }: { graphRevision?: number }) {
  const state = useCompatibleNodeCatalog({
    enabled: true,
    graphPath: 'events/Main.yssbi-event',
    graphRevision,
    sourcePort,
  });
  return createElement('output', {
    'data-status': state.status,
    'data-project': state.catalog?.projectInstanceId ?? '',
    'data-results': state.searchIndex?.search('').map((item) => item.nodeTypeId).join(',') ?? '',
  });
}

describe('useCompatibleNodeCatalog', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject('project-1', 7);
    useProjectIOStore.setState({ projectInstanceId: 'project-1' });
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    host.remove();
    projectPublicationCoordinator.cancelProject();
    useProjectIOStore.setState({ projectInstanceId: null });
  });

  it('requests exact graph authority and searches only backend-filtered items', async () => {
    vi.mocked(CatalogService.getCompatibleNodeCatalog)
      .mockResolvedValue(catalog('project-1', 'compatible.node'));

    await act(async () => root.render(createElement(Harness)));
    await vi.waitFor(() => expect(host.querySelector('output')?.dataset.status).toBe('ready'));

    expect(CatalogService.getCompatibleNodeCatalog).toHaveBeenCalledWith({
      projectInstanceId: 'project-1',
      graphPath: 'events/Main.yssbi-event',
      graphRevision: 4,
      sourcePort,
      locale: 'en-US',
    });
    expect(host.querySelector('output')?.dataset.results).toBe('compatible.node');
  });

  it('ignores a response from a stale project identity', async () => {
    const stale = deferred<LocalizedCatalogDto>();
    vi.mocked(CatalogService.getCompatibleNodeCatalog)
      .mockReturnValueOnce(stale.promise)
      .mockResolvedValueOnce(catalog('project-2', 'current.node'));

    await act(async () => root.render(createElement(Harness)));
    projectPublicationCoordinator.startProject('project-2', 7);
    await act(async () => {
      useProjectIOStore.setState({ projectInstanceId: 'project-2' });
      root.render(createElement(Harness));
    });
    await vi.waitFor(() => expect(host.querySelector('output')?.dataset.results).toBe('current.node'));

    stale.resolve(catalog('project-1', 'stale.node'));
    await act(async () => {
      await stale.promise;
      await Promise.resolve();
    });

    expect(host.querySelector('output')?.dataset).toMatchObject({
      status: 'ready',
      project: 'project-2',
      results: 'current.node',
    });
  });
});
