// @vitest-environment happy-dom
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { useNodeCatalogStore } from '@/features/core/nodeCatalog/nodeCatalogStore';
import { CatalogService, type LocalizedCatalogDto } from '@/services/nodeSystem/catalogService';
import { normalizeIpcError } from '@/services/ipc';
import { useLocalizedNodeCatalog } from './useLocalizedNodeCatalog';

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

vi.mock('@/services/nodeSystem/catalogService', () => ({
  CatalogService: { getLocalizedCatalog: vi.fn() },
}));

function catalog(projectInstanceId: string, locale: string): LocalizedCatalogDto {
  const nodeTypeId = `${projectInstanceId}-${locale}`;
  return {
    projectInstanceId,
    locale,
    registryFingerprint: `registry-${projectInstanceId}`,
    resourcePublicationRevision: 7,
    categories: [],
    items: [{
      nodeTypeId,
      title: nodeTypeId,
      documentation: null,
      categoryId: 'tests',
      iconId: 'tests',
      styleId: 'default',
      aliases: [],
      technicalTerms: [],
      backendSearchText: [nodeTypeId],
      resourceNames: [],
      ports: [],
      parameters: [],
      creation: { kind: 'static', nodeTypeId },
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

function Harness() {
  const state = useLocalizedNodeCatalog();
  return createElement('output', {
    'data-status': state.status,
    'data-error-code': state.error?.code ?? '',
    'data-incident-id': state.error?.incidentId ?? '',
    'data-project': state.catalog?.projectInstanceId ?? '',
    'data-locale': state.catalog?.locale ?? '',
    'data-results': state.searchIndex?.search('').length ?? 0,
    onClick: state.refresh,
  });
}

describe('useLocalizedNodeCatalog', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    localeState.language = 'zh-CN';
    useNodeCatalogStore.getState().clear();
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

  it('loads the initial project catalog and exposes its search index', async () => {
    vi.mocked(CatalogService.getLocalizedCatalog).mockResolvedValue(catalog('project-1', 'zh-CN'));

    await act(async () => root.render(createElement(Harness)));
    await vi.waitFor(() => expect(host.querySelector('output')?.dataset.status).toBe('ready'));

    expect(CatalogService.getLocalizedCatalog).toHaveBeenCalledWith('project-1', 'zh-CN');
    expect(host.querySelector('output')?.dataset).toMatchObject({
      project: 'project-1',
      locale: 'zh-CN',
      results: '1',
    });
  });

  it('loads a separate catalog when the locale changes', async () => {
    vi.mocked(CatalogService.getLocalizedCatalog)
      .mockResolvedValueOnce(catalog('project-1', 'zh-CN'))
      .mockResolvedValueOnce(catalog('project-1', 'en-US'));

    await act(async () => root.render(createElement(Harness)));
    await vi.waitFor(() => expect(host.querySelector('output')?.dataset.status).toBe('ready'));

    localeState.language = 'en-US';
    await act(async () => root.render(createElement(Harness)));
    await vi.waitFor(() => expect(host.querySelector('output')?.dataset.locale).toBe('en-US'));

    expect(CatalogService.getLocalizedCatalog).toHaveBeenNthCalledWith(2, 'project-1', 'en-US');
  });

  it('reuses a ready cached catalog when the hook remounts', async () => {
    vi.mocked(CatalogService.getLocalizedCatalog).mockResolvedValue(catalog('project-1', 'zh-CN'));

    await act(async () => root.render(createElement(Harness, { key: 'first' })));
    await vi.waitFor(() => expect(host.querySelector('output')?.dataset.status).toBe('ready'));
    await act(async () => root.render(createElement(Harness, { key: 'second' })));

    expect(CatalogService.getLocalizedCatalog).toHaveBeenCalledTimes(1);
    expect(host.querySelector('output')?.dataset.project).toBe('project-1');
  });

  it('shares an in-flight request with a remounted consumer', async () => {
    const pending = deferred<LocalizedCatalogDto>();
    vi.mocked(CatalogService.getLocalizedCatalog).mockReturnValue(pending.promise);

    await act(async () => root.render(createElement(Harness, { key: 'first' })));
    await vi.waitFor(() => expect(host.querySelector('output')?.dataset.status).toBe('loading'));
    await act(async () => root.render(createElement(Harness, { key: 'second' })));

    expect(CatalogService.getLocalizedCatalog).toHaveBeenCalledTimes(1);
    pending.resolve(catalog('project-1', 'zh-CN'));
    await act(async () => {
      await pending.promise;
      await Promise.resolve();
    });
    expect(host.querySelector('output')?.dataset.status).toBe('ready');
    expect(host.querySelector('output')?.dataset.project).toBe('project-1');
  });

  it('does not let a stale project response replace the current catalog', async () => {
    const stale = deferred<LocalizedCatalogDto>();
    vi.mocked(CatalogService.getLocalizedCatalog)
      .mockReturnValueOnce(stale.promise)
      .mockResolvedValueOnce(catalog('project-2', 'zh-CN'));

    await act(async () => root.render(createElement(Harness)));
    projectPublicationCoordinator.startProject('project-2', 7);
    await act(async () => {
      useProjectIOStore.setState({ projectInstanceId: 'project-2' });
      root.render(createElement(Harness));
    });
    await vi.waitFor(() => expect(host.querySelector('output')?.dataset.project).toBe('project-2'));

    stale.resolve(catalog('project-1', 'zh-CN'));
    await act(async () => {
      await stale.promise;
      await Promise.resolve();
    });

    const output = host.querySelector('output');
    expect(output?.dataset.status).toBe('ready');
    expect(output?.dataset.errorCode).toBe('');
    expect(output?.dataset.project).toBe('project-2');
  });

  it('refetches to the exact resource publication watermark and keeps the old catalog while loading', async () => {
    const refresh = deferred<LocalizedCatalogDto>();
    vi.mocked(CatalogService.getLocalizedCatalog)
      .mockResolvedValueOnce(catalog('project-1', 'zh-CN'))
      .mockReturnValueOnce(refresh.promise);

    await act(async () => root.render(createElement(Harness)));
    await vi.waitFor(() => expect(host.querySelector('output')?.dataset.status).toBe('ready'));

    act(() => {
      useNodeCatalogStore.getState().observeResourcePublication('project-1', 8);
    });
    await vi.waitFor(() => expect(CatalogService.getLocalizedCatalog).toHaveBeenCalledTimes(2));
    expect(host.querySelector('output')?.dataset).toMatchObject({
      status: 'loading',
      project: 'project-1',
    });

    const current = catalog('project-1', 'zh-CN');
    current.resourcePublicationRevision = 8;
    refresh.resolve(current);
    await act(async () => {
      await refresh.promise;
      await Promise.resolve();
    });
    expect(host.querySelector('output')?.dataset.status).toBe('ready');
    expect(useNodeCatalogStore.getState().projectWatermarks['project-1']).toBe(8);
  });

  it('preserves the last catalog when a publication refresh fails and allows an explicit retry', async () => {
    vi.mocked(CatalogService.getLocalizedCatalog)
      .mockResolvedValueOnce(catalog('project-1', 'zh-CN'))
      .mockRejectedValueOnce(new Error('refresh unavailable'))
      .mockResolvedValueOnce({ ...catalog('project-1', 'zh-CN'), resourcePublicationRevision: 8 });

    await act(async () => root.render(createElement(Harness)));
    await vi.waitFor(() => expect(host.querySelector('output')?.dataset.status).toBe('ready'));
    await act(async () => {
      useNodeCatalogStore.getState().observeResourcePublication('project-1', 8);
      await vi.waitFor(() => expect(
        useNodeCatalogStore.getState().requests['["project-1","zh-CN"]']?.status,
      ).toBe('error'));
    });

    expect(host.querySelector('output')?.dataset).toMatchObject({
      errorCode: 'catalog_response_contract_error',
      project: 'project-1',
      results: '1',
    });
    expect(host.innerHTML).not.toContain('refresh unavailable');

    await act(async () => {
      host.querySelector('output')?.click();
      await vi.waitFor(() => expect(
        useNodeCatalogStore.getState().requests['["project-1","zh-CN"]']?.status,
      ).toBe('ready'));
    });
    expect(CatalogService.getLocalizedCatalog).toHaveBeenCalledTimes(3);
  });

  it('maps parser prose to an explicit contract code for the current project request', async () => {
    vi.mocked(CatalogService.getLocalizedCatalog).mockRejectedValue(
      new Error('private localized catalog parser prose'),
    );

    await act(async () => root.render(createElement(Harness)));
    await vi.waitFor(() => expect(host.querySelector('output')?.dataset.status).toBe('error'));

    expect(host.querySelector('output')?.dataset.errorCode).toBe(
      'catalog_response_contract_error',
    );
    expect(host.innerHTML).not.toContain('private localized catalog parser prose');
  });

  it('keeps only the normalized transport code and drops transport prose', async () => {
    vi.mocked(CatalogService.getLocalizedCatalog).mockRejectedValue(
      normalizeIpcError(
        'get_localized_node_catalog',
        new Error('private localized catalog transport prose'),
      ),
    );

    await act(async () => root.render(createElement(Harness)));
    await vi.waitFor(() => expect(host.querySelector('output')?.dataset.status).toBe('error'));

    expect(host.querySelector('output')?.dataset.errorCode).toBe('ipc_transport_failure');
    expect(host.innerHTML).not.toContain('private localized catalog transport prose');
  });

  it('preserves backend code and incident ID without retaining backend details', async () => {
    vi.mocked(CatalogService.getLocalizedCatalog).mockRejectedValue(
      normalizeIpcError('get_localized_node_catalog', {
        code: 'catalog_backend_failed',
        details: { debug: 'private catalog backend detail' },
        incidentId: 'incident-catalog-42',
      }),
    );

    await act(async () => root.render(createElement(Harness)));
    await vi.waitFor(() => expect(host.querySelector('output')?.dataset.status).toBe('error'));

    expect(host.querySelector('output')?.dataset).toMatchObject({
      errorCode: 'catalog_backend_failed',
      incidentId: 'incident-catalog-42',
    });
    expect(host.innerHTML).not.toContain('private catalog backend detail');
  });
});
