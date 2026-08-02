import { describe, expect, it, vi } from 'vitest';
import type { LocalizedCatalogItem } from './catalogItem';
import { isLocalizedCatalogItem } from './catalogItem';
import { searchLocalizedCatalogItems } from './search';

function item(overrides: Partial<LocalizedCatalogItem> = {}): LocalizedCatalogItem {
  return {
    nodeTypeId: 'yssbi.example.node',
    title: 'Localized title',
    description: null,
    documentation: null,
    categoryId: 'examples',
    aliases: [],
    technicalTerms: [],
    creation: { kind: 'static', nodeTypeId: 'yssbi.example.node' },
    searchText: '',
    ...overrides,
  };
}

describe('searchLocalizedCatalogItems', () => {
  it.each([
    ['localized title', item({ nodeTypeId: 'title', title: '整数相加' }), '整数'],
    ['alias', item({ nodeTypeId: 'alias', aliases: ['求和'] }), '求和'],
    ['technical term', item({ nodeTypeId: 'technical', technicalTerms: ['Int64'] }), 'int64'],
    [
      'stable ID in backend search text',
      item({ nodeTypeId: 'stable-id', searchText: 'yssbi numeric add int64' }),
      'yssbi.numeric.add',
    ],
    ['pinyin', item({ nodeTypeId: 'pinyin', pinyin: 'zheng shu xiang jia' }), 'xiang jia'],
  ])('matches %s', (_field, catalogItem, query) => {
    expect(searchLocalizedCatalogItems([catalogItem], query)).toEqual([catalogItem]);
  });

  it('matches ASCII identifiers independently of the host locale', () => {
    const originalLocaleLowerCase = String.prototype.toLocaleLowerCase;
    const localeLowerCase = vi
      .spyOn(String.prototype, 'toLocaleLowerCase')
      .mockImplementation(function (this: string) {
        return originalLocaleLowerCase.call(this, 'tr-TR');
      });
    const technicalItem = item({ nodeTypeId: 'technical', technicalTerms: ['Int64'] });
    const stableIdItem = item({
      nodeTypeId: 'stable-id',
      searchText: 'yssbi Integration Node',
    });

    try {
      expect(searchLocalizedCatalogItems([technicalItem], 'int64')).toEqual([technicalItem]);
      expect(searchLocalizedCatalogItems([stableIdItem], 'integration.node')).toEqual([stableIdItem]);
    } finally {
      localeLowerCase.mockRestore();
    }
  });

  it('does not search descriptions or documentation', () => {
    const catalogItem = item({
      description: 'description-only-secret',
      documentation: 'documentation-only-secret',
    });

    expect(searchLocalizedCatalogItems([catalogItem], 'description-only-secret')).toEqual([]);
    expect(searchLocalizedCatalogItems([catalogItem], 'documentation-only-secret')).toEqual([]);
  });

  it('accepts complete static catalog items and rejects unsupported creation descriptors', () => {
    const catalogItem = item();
    const unsupported = {
      ...catalogItem,
      creation: {
        kind: 'resourceBound',
        nodeTypeId: catalogItem.nodeTypeId,
      },
    };

    expect(isLocalizedCatalogItem(catalogItem)).toBe(true);
    expect(isLocalizedCatalogItem(unsupported)).toBe(false);
  });
});
