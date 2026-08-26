import { describe, expect, it, vi } from 'vitest';
import type { LocalizedCatalogItem } from './catalogItem';
import { isLocalizedCatalogItem } from './catalogItem';
import { buildCatalogSearchDocument } from './searchDocument';
import { searchLocalizedCatalogItems } from './search';

function item(overrides: Partial<LocalizedCatalogItem> = {}): LocalizedCatalogItem {
  return Object.assign({
    nodeTypeId: 'yssbi.example.node',
    title: 'Localized title',
    documentation: null,
    categoryId: 'examples',
    iconId: 'example',
    styleId: 'default',
    aliases: [],
    technicalTerms: [],
    ports: [],
    parameters: [],
    creation: { kind: 'static' as const, nodeTypeId: 'yssbi.example.node' },
    backendSearchText: [],
    resourceNames: [],
  }, overrides);
}

describe('searchLocalizedCatalogItems', () => {
  it.each([
    ['localized title', item({ nodeTypeId: 'title', title: '整数相加' }), '整数'],
    ['alias', item({ nodeTypeId: 'alias', aliases: ['求和'] }), '求和'],
    ['technical term', item({ nodeTypeId: 'technical', technicalTerms: ['Int64'] }), 'int64'],
    ['technical term full pinyin', item({ nodeTypeId: 'technical-pinyin', technicalTerms: ['技术术语'] }), 'ji shu shu yu'],
    ['technical term pinyin initials', item({ nodeTypeId: 'technical-initials', technicalTerms: ['技术术语'] }), 'jssy'],
    ['stable node ID', item({ nodeTypeId: 'yssbi.hidden.stable-id' }), 'hidden.stable-id'],
    ['backend search text', item({ nodeTypeId: 'backend', backendSearchText: ['backend-only-secret'] }), 'backend-only-secret'],
    ['resource name', item({ nodeTypeId: 'resource', resourceNames: ['季度销售数据'] }), '季度销售'],
    ['full pinyin', item({ nodeTypeId: 'full', title: '整数相加' }), 'zheng shu xiang jia'],
    ['pinyin initials', item({ nodeTypeId: 'initials', title: '整数相加' }), 'zsxj'],
    ['mixed Chinese and Latin', item({ nodeTypeId: 'mixed', title: '整数 AddParser' }), 'zs addparser'],
  ])('matches %s', (_field, catalogItem, query) => {
    expect(searchLocalizedCatalogItems([catalogItem], query)).toEqual([catalogItem]);
  });

  it.each([
    ['documentation', item({ documentation: 'documentation-only-secret' }), 'documentation-only-secret'],
  ])('does not match %s outside the search document', (_field, catalogItem, query) => {
    expect(searchLocalizedCatalogItems([catalogItem], query)).toEqual([]);
  });

  it('builds deterministic polyphonic pinyin and preserves unknown characters', () => {
    const document = buildCatalogSearchDocument(item({
      nodeTypeId: 'yssbi.polyphonic.fixture',
      title: '重庆银行',
      aliases: ['整数Add🧪'],
      technicalTerms: ['技术术语'],
      backendSearchText: ['未知𠮷A'],
      resourceNames: ['数据源DB'],
    }));

    expect(document).toEqual({
      nodeTypeId: 'yssbi.polyphonic.fixture',
      localizedTitle: '重庆银行',
      aliases: ['整数add'],
      technicalTerms: ['技术术语'],
      backendSearchText: ['未知𠮷a'],
      resourceNames: ['数据源db'],
      pinyinFull: [
        'chong qing yin hang',
        'zheng shu add',
        'ji shu shu yu',
        'wei zhi 𠮷a',
        'shu ju yuan db',
      ],
      pinyinInitials: ['cqyh', 'zsadd', 'jssy', 'wz𠮷a', 'sjydb'],
    });
  });

  it('normalizes raw wire metadata once across Unicode boundary cases', () => {
    const catalogItem = item({
      title: 'Straße_Value',
      aliases: ['Cafe\u0301_ALIAS'],
      technicalTerms: ['Ångström_TERM'],
      backendSearchText: ['Ｍaße_Backend'],
      resourceNames: ['资源_Name'],
    });

    expect(buildCatalogSearchDocument(catalogItem)).toMatchObject({
      localizedTitle: 'straße value',
      aliases: ['cafe alias'],
      technicalTerms: ['angstrom term'],
      backendSearchText: ['maße backend'],
      resourceNames: ['资源 name'],
    });
    for (const query of ['straße value', 'cafe alias', 'angstrom term', 'maße backend', 'zi yuan name']) {
      expect(searchLocalizedCatalogItems([catalogItem], query)).toEqual([catalogItem]);
    }
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
