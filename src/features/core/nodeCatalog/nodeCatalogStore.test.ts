import { beforeEach, describe, expect, it } from 'vitest';
import {
  catalogResponseKey,

  useNodeCatalogStore,
  type LocalizedCatalogResponse,
} from './nodeCatalogStore';

function catalog(
  overrides: Partial<Pick<
    LocalizedCatalogResponse,
    'projectInstanceId' | 'locale' | 'registryFingerprint' | 'resourcePublicationRevision'
  >> = {},
): LocalizedCatalogResponse {
  return {
    projectInstanceId: 'project-1',
    locale: 'zh-CN',
    registryFingerprint: 'registry-1',
    resourcePublicationRevision: 7,
    categories: [],
    items: [],
    ...overrides,
  };
}

describe('useNodeCatalogStore', () => {
  beforeEach(() => useNodeCatalogStore.getState().clear());

  it.each([
    ['project instance ID', { projectInstanceId: 'project-2' }],
    ['locale', { locale: 'en-US' }],
    ['Registry fingerprint', { registryFingerprint: 'registry-2' }],
    ['resource publication revision', { resourcePublicationRevision: 8 }],
  ] as const)('retains distinct cached responses by %s', (_dimension, overrides) => {
    const baseline = catalog();
    const changed = catalog(overrides);
    const store = useNodeCatalogStore.getState();
    const baselineOwner = store.beginRequest(baseline.projectInstanceId, baseline.locale);
    expect(baselineOwner).toBeDefined();
    expect(store.storeResponse(baselineOwner!, baseline)).toBe(true);
    const changedOwner = store.beginRequest(changed.projectInstanceId, changed.locale);
    expect(changedOwner).toBeDefined();
    expect(store.storeResponse(changedOwner!, changed)).toBe(true);

    const { responses } = useNodeCatalogStore.getState();
    expect(responses[catalogResponseKey(baseline)]).toBe(baseline);
    expect(responses[catalogResponseKey(changed)]).toBe(changed);
    expect(Object.keys(responses)).toHaveLength(2);
  });

  it('replaces equal-metadata cached response provenance with the latest DTO object', () => {
    const first = catalog();
    const second = catalog();
    first.items = [];
    second.items = [{
      nodeTypeId: 'second', title: 'Second', documentation: null,
      categoryId: 'tests', iconId: 'tests', styleId: 'default', aliases: [],
      technicalTerms: [], backendSearchText: [], resourceNames: [], ports: [], parameters: [],
      creation: { kind: 'static', nodeTypeId: 'second' },
    }];
    const store = useNodeCatalogStore.getState();

    expect(store.storeResponse(store.beginRequest('project-1', 'zh-CN')!, first)).toBe(true);
    expect(store.storeResponse(store.beginRequest('project-1', 'zh-CN')!, second)).toBe(true);

    expect(useNodeCatalogStore.getState().responses[catalogResponseKey(second)]).toBe(second);
  });

  it('deduplicates an exact project and locale while its owner is loading', () => {
    const response = catalog();
    const store = useNodeCatalogStore.getState();
    const owner = store.beginRequest(response.projectInstanceId, response.locale);

    expect(owner).toBeDefined();
    expect(store.beginRequest(response.projectInstanceId, response.locale)).toBeNull();
    expect(useNodeCatalogStore.getState().responses).toEqual({});
    expect(useNodeCatalogStore.getState().requests).toEqual({
      '["project-1","zh-CN"]': {
        status: 'loading',
        responseKey: null,
        error: null,
        requestGeneration: owner!.requestGeneration,
        minimumResourcePublicationRevision: 0,
      },
    });
  });

  it('keeps newer success when an older owner errors later', () => {
    const response = catalog({ registryFingerprint: 'registry-new', resourcePublicationRevision: 8 });
    const store = useNodeCatalogStore.getState();
    const older = store.beginRequest(response.projectInstanceId, response.locale)!;
    store.clear();
    const newer = store.beginRequest(response.projectInstanceId, response.locale)!;

    expect(store.storeResponse(newer, response)).toBe(true);
    expect(store.storeError(older, {
      code: 'catalog_request_failed',
      incidentId: null,
    })).toBe(false);
    expect(useNodeCatalogStore.getState().responses).toEqual({
      [catalogResponseKey(response)]: response,
    });
    expect(useNodeCatalogStore.getState().requests).toEqual({
      '["project-1","zh-CN"]': {
        status: 'ready',
        responseKey: catalogResponseKey(response),
        error: null,
        requestGeneration: newer.requestGeneration,
        minimumResourcePublicationRevision: 0,
      },
    });
  });

  it('rejects an out-of-order metadata response from an older owner', () => {
    const olderResponse = catalog({ registryFingerprint: 'registry-old', resourcePublicationRevision: 7 });
    const newerResponse = catalog({ registryFingerprint: 'registry-new', resourcePublicationRevision: 8 });
    const store = useNodeCatalogStore.getState();
    const older = store.beginRequest('project-1', 'zh-CN')!;
    store.clear();
    const newer = store.beginRequest('project-1', 'zh-CN')!;

    expect(store.storeResponse(newer, newerResponse)).toBe(true);
    expect(store.storeResponse(older, olderResponse)).toBe(false);
    expect(useNodeCatalogStore.getState().responses).toEqual({
      [catalogResponseKey(newerResponse)]: newerResponse,
    });
    expect(useNodeCatalogStore.getState().requests).toEqual({
      '["project-1","zh-CN"]': {
        status: 'ready',
        responseKey: catalogResponseKey(newerResponse),
        error: null,
        requestGeneration: newer.requestGeneration,
        minimumResourcePublicationRevision: 0,
      },
    });
  });

  it('stores a stable contract code when response identity does not match its request', () => {
    const store = useNodeCatalogStore.getState();
    const owner = store.beginRequest('project-1', 'zh-CN')!;

    expect(store.storeResponse(owner, catalog({ projectInstanceId: 'project-2' }))).toBe(false);
    expect(useNodeCatalogStore.getState().requests['["project-1","zh-CN"]']).toMatchObject({
      status: 'error',
      error: {
        code: 'catalog_response_contract_error',
        incidentId: null,
      },
    });
  });

  it('invalidates only the matching project when its publication watermark advances', () => {
    const projectOne = catalog();
    const projectTwo = catalog({ projectInstanceId: 'project-2' });
    const store = useNodeCatalogStore.getState();
    const ownerOne = store.beginRequest(projectOne.projectInstanceId, projectOne.locale)!;
    const ownerTwo = store.beginRequest(projectTwo.projectInstanceId, projectTwo.locale)!;
    store.storeResponse(ownerOne, projectOne);
    store.storeResponse(ownerTwo, projectTwo);

    expect(store.observeResourcePublication('project-1', 8)).toBe(true);

    const state = useNodeCatalogStore.getState();
    expect(state.requests['["project-1","zh-CN"]']).toMatchObject({
      status: 'idle',
      responseKey: catalogResponseKey(projectOne),
      minimumResourcePublicationRevision: 8,
    });
    expect(state.requests['["project-2","zh-CN"]']).toMatchObject({
      status: 'ready',
      responseKey: catalogResponseKey(projectTwo),
      minimumResourcePublicationRevision: 0,
    });
    expect(store.observeResourcePublication('project-1', 8)).toBe(false);
  });

  it('rejects a response below the requested publication watermark without losing cached data', () => {
    const previous = catalog({ resourcePublicationRevision: 7 });
    const store = useNodeCatalogStore.getState();
    store.storeResponse(store.beginRequest('project-1', 'zh-CN')!, previous);
    store.observeResourcePublication('project-1', 9);
    const refresh = store.beginRequest('project-1', 'zh-CN')!;

    expect(store.storeResponse(refresh, catalog({ resourcePublicationRevision: 8 }))).toBe(false);
    expect(useNodeCatalogStore.getState().requests['["project-1","zh-CN"]']).toMatchObject({
      status: 'error',
      responseKey: catalogResponseKey(previous),
      error: {
        code: 'catalog_response_stale',
        incidentId: null,
      },
      minimumResourcePublicationRevision: 9,
    });
  });

  it('preserves the last ready response when a refresh fails', () => {
    const previous = catalog();
    const store = useNodeCatalogStore.getState();
    store.storeResponse(store.beginRequest('project-1', 'zh-CN')!, previous);
    store.observeResourcePublication('project-1', 8);
    const refresh = store.beginRequest('project-1', 'zh-CN')!;

    expect(store.storeError(refresh, {
      code: 'catalog_refresh_failed',
      incidentId: 'incident-catalog-refresh',
    })).toBe(true);
    expect(useNodeCatalogStore.getState().requests['["project-1","zh-CN"]']).toMatchObject({
      status: 'error',
      responseKey: catalogResponseKey(previous),
      error: {
        code: 'catalog_refresh_failed',
        incidentId: 'incident-catalog-refresh',
      },
      minimumResourcePublicationRevision: 8,
    });
  });
});
