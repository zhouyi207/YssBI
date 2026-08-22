import type { LocalizedSearchIndex } from '@/features/core/nodeCatalog/localizedSearchIndex';
import type { LocalizedCatalogResponse } from '@/features/core/nodeCatalog/nodeCatalogStore';
import {
  buildLocalizedCatalogTree,
  collectLocalizedCatalogCategoryIds,
  flattenLocalizedCatalogTree,
  type LocalizedCatalogBrowserRow,
} from '@/features/domain/nodeCatalog/localizedCatalogTree';

export interface LocalizedCatalogBrowserProjection {
  rows: LocalizedCatalogBrowserRow[];
  categoryIds: ReadonlySet<string>;
  expandedCategoryIds: ReadonlySet<string>;
}

export function buildLocalizedCatalogBrowser({
  catalog,
  searchIndex,
  query,
  expandedCategoryIds,
}: {
  catalog: LocalizedCatalogResponse | null;
  searchIndex: LocalizedSearchIndex | null;
  query: string;
  expandedCategoryIds: ReadonlySet<string>;
}): LocalizedCatalogBrowserProjection {
  const searching = query.trim().length > 0;
  const items = !catalog
    ? []
    : searching
      ? searchIndex?.search(query) ?? []
      : catalog.items;
  const tree = catalog
    ? buildLocalizedCatalogTree(catalog.categories, items)
    : [];
  const effectiveExpandedCategoryIds = searching
    ? collectLocalizedCatalogCategoryIds(tree)
    : expandedCategoryIds;

  return {
    categoryIds: collectLocalizedCatalogCategoryIds(tree),
    expandedCategoryIds: effectiveExpandedCategoryIds,
    rows: flattenLocalizedCatalogTree(tree, effectiveExpandedCategoryIds),
  };
}
