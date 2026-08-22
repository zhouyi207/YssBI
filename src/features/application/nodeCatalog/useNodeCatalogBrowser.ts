import { useCallback, useEffect, useMemo } from 'react';
import { useLocalizedNodeCatalog } from './useLocalizedNodeCatalog';
import { buildLocalizedCatalogBrowser } from './catalogTreeBrowser';
import type { LocalizedCatalogBrowserRow } from '@/features/domain/nodeCatalog/localizedCatalogTree';
import { useNodeCatalogTreeStore } from '@/features/core/nodeCatalog/nodeCatalogTreeStore';

const EMPTY_CATEGORY_IDS = new Set<string>();

export interface NodeCatalogBrowserState {
  status: ReturnType<typeof useLocalizedNodeCatalog>['status'];
  error: ReturnType<typeof useLocalizedNodeCatalog>['error'];
  catalog: ReturnType<typeof useLocalizedNodeCatalog>['catalog'];
  query: string;
  queryIsActive: boolean;
  rows: LocalizedCatalogBrowserRow[];
  allCategoriesExpanded: boolean;
  canToggleAllCategories: boolean;
  expandedCategoryIds: ReadonlySet<string>;
  setQuery: (query: string) => void;
  setCategoryExpanded: (categoryId: string, expanded: boolean) => void;
  toggleAllCategories: () => void;
}

export function useNodeCatalogBrowser(): NodeCatalogBrowserState {
  const localized = useLocalizedNodeCatalog();
  const storeScopeKey = useNodeCatalogTreeStore((state) => state.scopeKey);
  const storedQuery = useNodeCatalogTreeStore((state) => state.query);
  const storedExpandedCategoryIds = useNodeCatalogTreeStore(
    (state) => state.expandedCategoryIds,
  );
  const setScope = useNodeCatalogTreeStore((state) => state.setScope);
  const setQuery = useNodeCatalogTreeStore((state) => state.setQuery);
  const setCategoryExpanded = useNodeCatalogTreeStore((state) => state.setCategoryExpanded);
  const setCategoriesExpanded = useNodeCatalogTreeStore((state) => state.setCategoriesExpanded);

  const scopeKey = localized.catalog
    ? JSON.stringify([
        localized.catalog.projectInstanceId,
        localized.catalog.locale,
        localized.catalog.registryFingerprint,
      ])
    : null;

  useEffect(() => {
    setScope(scopeKey);
  }, [scopeKey, setScope]);

  const query = storeScopeKey === scopeKey ? storedQuery : '';
  const manualExpandedCategoryIds = storeScopeKey === scopeKey
    ? storedExpandedCategoryIds
    : EMPTY_CATEGORY_IDS;
  const projection = useMemo(
    () => buildLocalizedCatalogBrowser({
      catalog: localized.catalog,
      searchIndex: localized.searchIndex,
      query,
      expandedCategoryIds: manualExpandedCategoryIds,
    }),
    [localized.catalog, localized.searchIndex, manualExpandedCategoryIds, query],
  );
  const queryIsActive = query.trim().length > 0;
  const allCategoriesExpanded = !queryIsActive
    && projection.categoryIds.size > 0
    && [...projection.categoryIds].every((categoryId) => (
      manualExpandedCategoryIds.has(categoryId)
    ));
  const canToggleAllCategories = !queryIsActive && projection.categoryIds.size > 0;
  const setVisibleCategoryExpanded = useCallback((categoryId: string, expanded: boolean) => {
    if (queryIsActive) return;
    setCategoryExpanded(categoryId, expanded);
  }, [queryIsActive, setCategoryExpanded]);
  const toggleAllCategories = useCallback(() => {
    if (!canToggleAllCategories) return;
    setCategoriesExpanded(projection.categoryIds, !allCategoriesExpanded);
  }, [allCategoriesExpanded, canToggleAllCategories, projection.categoryIds, setCategoriesExpanded]);

  return {
    status: localized.status,
    error: localized.error,
    catalog: localized.catalog,
    query,
    queryIsActive,
    rows: projection.rows,
    allCategoriesExpanded,
    canToggleAllCategories,
    expandedCategoryIds: projection.expandedCategoryIds,
    setQuery,
    setCategoryExpanded: setVisibleCategoryExpanded,
    toggleAllCategories,
  };
}
