import { describe, expect, it } from 'vitest';
import type {
  LocalizedCatalogItemDto,
  LocalizedCategoryDto,
} from '@/shared/types/dto/localizedCatalog';
import { buildLocalizedCatalogTree } from './localizedCatalogTree';

const categories: LocalizedCategoryDto[] = [
  {
    categoryId: 'statistics.regression',
    parentCategoryId: 'statistics',
    order: 11,
    title: 'Regression',
    searchText: 'regression',
  },
  {
    categoryId: 'output',
    parentCategoryId: null,
    order: 20,
    title: 'Output',
    searchText: 'output',
  },
  {
    categoryId: 'statistics',
    parentCategoryId: null,
    order: 10,
    title: 'Statistics',
    searchText: 'statistics',
  },
];

function item(
  nodeTypeId: string,
  categoryId: string,
): LocalizedCatalogItemDto {
  return {
    nodeTypeId,
    title: nodeTypeId,
    description: null,
    documentation: null,
    categoryId,
    iconId: 'test',
    styleId: 'default',
    aliases: [],
    technicalTerms: [],
    backendSearchText: [],
    resourceNames: [],
    ports: [],
    parameters: [],
    creation: { kind: 'static', nodeTypeId },
  };
}

describe('buildLocalizedCatalogTree', () => {
  it('builds an ordered hierarchy independently of category input order', () => {
    const tree = buildLocalizedCatalogTree(categories, [
      item('output.print', 'output'),
      item('statistics.logit.fit', 'statistics.regression'),
    ]);

    expect(tree.map((node) => node.category.categoryId)).toEqual([
      'statistics',
      'output',
    ]);
    expect(tree[0].children.map((node) => node.category.categoryId)).toEqual([
      'statistics.regression',
    ]);
    expect(tree[0].children[0].items.map((entry) => entry.nodeTypeId)).toEqual([
      'statistics.logit.fit',
    ]);
  });

  it('omits empty branches while retaining ancestors of matching items', () => {
    const tree = buildLocalizedCatalogTree(categories, [
      item('statistics.logit.fit', 'statistics.regression'),
    ]);

    expect(tree).toHaveLength(1);
    expect(tree[0].category.categoryId).toBe('statistics');
    expect(tree[0].children[0].category.categoryId).toBe('statistics.regression');
  });
});
