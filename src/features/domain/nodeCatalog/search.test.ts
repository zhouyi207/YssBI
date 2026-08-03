import { describe, expect, it, vi } from 'vitest';
import type { LocalizedCatalogItem } from './catalogItem';
import { isLocalizedCatalogItem } from './catalogItem';
import { searchLocalizedCatalogItems } from './search';

function item(overrides: Partial<LocalizedCatalogItem> = {}): LocalizedCatalogItem {
  return Object.assign({
    nodeTypeId: 'yssbi.example.node',
    title: 'Localized title',
    description: null,
    documentation: null,
    categoryId: 'examples',
    iconId: 'example',
    styleId: 'default',
    aliases: [],
    technicalTerms: [],
    ports: [],
    parameters: [],
    creation: { kind: 'static' as const, nodeTypeId: 'yssbi.example.node' },
    searchText: '',
  }, overrides);
}

describe('searchLocalizedCatalogItems', () => {
  it.each([
    ['localized title', item({ nodeTypeId: 'title', title: '整数相加' }), '整数'],
    ['alias', item({ nodeTypeId: 'alias', aliases: ['求和'] }), '求和'],
  ])('matches %s', (_field, catalogItem, query) => {
    expect(searchLocalizedCatalogItems([catalogItem], query)).toEqual([catalogItem]);
  });

  it.each([
    ['description', item({ description: 'description-only-secret' }), 'description-only-secret'],
    ['documentation', item({ documentation: 'documentation-only-secret' }), 'documentation-only-secret'],
    ['technical term', item({ technicalTerms: ['Int64'] }), 'int64'],
    ['stable node ID', item({ nodeTypeId: 'yssbi.hidden.stable-id' }), 'hidden.stable-id'],
    ['backend search text', item({ searchText: 'backend-only-secret' }), 'backend-only-secret'],
    ['pinyin', item({ pinyin: 'zheng shu xiang jia' }), 'xiang jia'],
  ])('does not match %s metadata', (_field, catalogItem, query) => {
    expect(searchLocalizedCatalogItems([catalogItem], query)).toEqual([]);
  });

  it('normalizes title and alias ASCII independently of the host locale', () => {
    const originalLocaleLowerCase = String.prototype.toLocaleLowerCase;
    const localeLowerCase = vi
      .spyOn(String.prototype, 'toLocaleLowerCase')
      .mockImplementation(function (this: string) {
        return originalLocaleLowerCase.call(this, 'tr-TR');
      });
    const titleItem = item({ title: 'Integration Node' });
    const aliasItem = item({ aliases: ['Int64'] });

    try {
      expect(searchLocalizedCatalogItems([titleItem], 'integration')).toEqual([titleItem]);
      expect(searchLocalizedCatalogItems([aliasItem], 'int64')).toEqual([aliasItem]);
    } finally {
      localeLowerCase.mockRestore();
    }
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
