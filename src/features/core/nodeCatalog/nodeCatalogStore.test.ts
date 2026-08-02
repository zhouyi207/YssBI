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
    expect(store.storeError(older, 'older request failed')).toBe(false);
    expect(useNodeCatalogStore.getState().responses).toEqual({
      [catalogResponseKey(response)]: response,
    });
    expect(useNodeCatalogStore.getState().requests).toEqual({
      '["project-1","zh-CN"]': {
        status: 'ready',
        responseKey: catalogResponseKey(response),
        error: null,
        requestGeneration: newer.requestGeneration,
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
      },
    });
  });
});
