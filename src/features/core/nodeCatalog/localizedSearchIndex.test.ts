import { describe, expect, it } from 'vitest';
import { getLocalizedSearchIndex } from './localizedSearchIndex';
import type { LocalizedCatalogResponse } from './nodeCatalogStore';

function catalog(
  overrides: Partial<Pick<
    LocalizedCatalogResponse,
    'projectInstanceId' | 'locale' | 'registryFingerprint' | 'resourcePublicationRevision'
  >> = {},
): LocalizedCatalogResponse {
  const nodeTypeId = [
    overrides.projectInstanceId ?? 'project-1',
    overrides.locale ?? 'zh-CN',
    overrides.registryFingerprint ?? 'registry-1',
    overrides.resourcePublicationRevision ?? 7,
  ].join('-');
  return {
    projectInstanceId: 'project-1',
    locale: 'zh-CN',
    registryFingerprint: 'registry-1',
    resourcePublicationRevision: 7,
    categories: [],
    items: [{
      nodeTypeId,
      title: nodeTypeId,
      description: null,
      documentation: null,
      categoryId: 'tests',
      iconId: 'tests',
      styleId: 'default',
      aliases: [],
      technicalTerms: [],
      ports: [],
      parameters: [],
      creation: { kind: 'static', nodeTypeId },
      searchText: nodeTypeId,
    }],
    ...overrides,
  };
}

describe('getLocalizedSearchIndex', () => {
  it.each([
    ['project instance ID', { projectInstanceId: 'project-2' }],
    ['locale', { locale: 'en-US' }],
    ['Registry fingerprint', { registryFingerprint: 'registry-2' }],
    ['resource publication revision', { resourcePublicationRevision: 8 }],
  ] as const)('isolates indexes by %s', (_dimension, overrides) => {
    const baselineResponse = catalog();
    const changedResponse = catalog(overrides);
    const baseline = getLocalizedSearchIndex(baselineResponse);
    const changed = getLocalizedSearchIndex(changedResponse);

    expect(changed).not.toBe(baseline);
    expect(changed.response).toBe(changedResponse);
    expect(changed.search(changed.response.items[0].nodeTypeId)).toEqual(changed.response.items);
  });

  it('reuses an index only when all response metadata matches exactly', () => {
    const first = catalog();
    const second = catalog();

    expect(getLocalizedSearchIndex(second)).toBe(getLocalizedSearchIndex(first));
  });

  it('indexes only current-locale item titles and aliases while preserving metadata', () => {
    const response = catalog({ resourcePublicationRevision: 99 });
    const item = response.items[0];
    item.title = '当前标题';
    item.aliases = ['current alias'];
    item.description = 'description-only-secret';
    item.documentation = 'documentation-only-secret';
    item.technicalTerms = ['technical-only-secret'];
    item.pinyin = 'pin yin secret';
    item.searchText = 'backend-search-only-secret';

    const index = getLocalizedSearchIndex(response);

    expect(index.search('当前')).toEqual([item]);
    expect(index.search('current alias')).toEqual([item]);
    for (const excluded of [
      'description-only-secret',
      'documentation-only-secret',
      'technical-only-secret',
      item.nodeTypeId,
      'pin yin secret',
      'backend-search-only-secret',
    ]) {
      expect(index.search(excluded)).toEqual([]);
    }
    expect(index.response.items[0]).toBe(item);
  });
});
