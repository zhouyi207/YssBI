import { useEffect, useMemo } from "react";
import { useLocalizedNodeCatalog } from "./useLocalizedNodeCatalog";
import { buildLocalizedCatalogBrowser } from "./catalogTreeBrowser";
import type { LocalizedCatalogBrowserRow } from "@/features/domain/nodeCatalog/localizedCatalogTree";
import { useNodeCatalogTreeStore } from "@/features/core/nodeCatalog/nodeCatalogTreeStore";

const EMPTY_CATEGORY_IDS = new Set<string>();

export interface NodeCatalogBrowserState {
  status: ReturnType<typeof useLocalizedNodeCatalog>["status"];
  error: ReturnType<typeof useLocalizedNodeCatalog>["error"];
  catalog: ReturnType<typeof useLocalizedNodeCatalog>["catalog"];
  rows: LocalizedCatalogBrowserRow[];
  expandedCategoryIds: ReadonlySet<string>;
  setCategoryExpanded: (categoryId: string, expanded: boolean) => void;
}

export function useNodeCatalogBrowser(): NodeCatalogBrowserState {
  const localized = useLocalizedNodeCatalog();
  const storeScopeKey = useNodeCatalogTreeStore((state) => state.scopeKey);
  const storedExpandedCategoryIds = useNodeCatalogTreeStore((state) => state.expandedCategoryIds);
  const setScope = useNodeCatalogTreeStore((state) => state.setScope);
  const setCategoryExpanded = useNodeCatalogTreeStore((state) => state.setCategoryExpanded);

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

  const manualExpandedCategoryIds =
    storeScopeKey === scopeKey ? storedExpandedCategoryIds : EMPTY_CATEGORY_IDS;
  const projection = useMemo(
    () =>
      buildLocalizedCatalogBrowser({
        catalog: localized.catalog,
        searchIndex: localized.searchIndex,
        query: "",
        expandedCategoryIds: manualExpandedCategoryIds,
      }),
    [localized.catalog, localized.searchIndex, manualExpandedCategoryIds],
  );

  return {
    status: localized.status,
    error: localized.error,
    catalog: localized.catalog,
    rows: projection.rows,
    expandedCategoryIds: projection.expandedCategoryIds,
    setCategoryExpanded,
  };
}
