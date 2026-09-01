import { useTranslation } from "react-i18next";
import { useNodeCatalogBrowser } from "@/features/application/nodeCatalog/useNodeCatalogBrowser";
import { nodeCatalogErrorText } from "@/features/application/nodeCatalog/nodeCatalogErrorPresentation";
import type { LocalizedCatalogBrowserRow } from "@/features/domain/nodeCatalog/localizedCatalogTree";
import {
  SidebarTabPanel,
  SidebarTreeSearchInput,
  SidebarVirtualTree,
  sidebarTreeSearchShellClass,
} from "@/modules/workbench/public";
import { SidebarCatalogTreeRow } from "./SidebarCatalogTreeRow";

const CATEGORY_ROW_HEIGHT = 28;
const ITEM_ROW_ESTIMATE = 32;

function rowEstimate(row: LocalizedCatalogBrowserRow | undefined): number {
  return row?.kind === "category" ? CATEGORY_ROW_HEIGHT : ITEM_ROW_ESTIMATE;
}

export function SidebarNodesTab() {
  const { t } = useTranslation();
  const {
    status,
    error,
    catalog,
    query,
    queryIsActive,
    rows,
    allCategoriesExpanded,
    canToggleAllCategories,
    expandedCategoryIds,
    setQuery,
    setCategoryExpanded,
    toggleAllCategories,
  } = useNodeCatalogBrowser();
  return (
    <SidebarTabPanel>
      {catalog ? (
        <div className={sidebarTreeSearchShellClass()}>
          <SidebarTreeSearchInput
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={t("canvas.nodePalette.searchPlaceholder")}
            expandAllLabel={t("canvas.nodePalette.expandAll")}
            collapseAllLabel={t("canvas.nodePalette.collapseAll")}
            allCategoriesExpanded={allCategoriesExpanded}
            canToggleAllCategories={canToggleAllCategories}
            onToggleAllCategories={toggleAllCategories}
          />
        </div>
      ) : null}
      {status === "error" && !catalog ? (
        <p role="alert" className="px-2 py-3 text-sm text-destructive">
          {nodeCatalogErrorText(error, t)}
        </p>
      ) : !catalog ? (
        <p role="status" className="px-2 py-3 text-sm text-muted-foreground">
          {t("common.loading")}
        </p>
      ) : (
        <SidebarVirtualTree
          rows={rows}
          ariaLabel={t("activityBar.nodes")}
          emptyMessage={t("sidebar.nodeSearchNoMatches")}
          getRowKey={(row) => row.rowKey}
          getRowDepth={(row) => row.depth}
          estimateSize={rowEstimate}
          renderRow={(row) => (
            <SidebarCatalogTreeRow
              row={row}
              expanded={row.kind === "category" && expandedCategoryIds.has(row.category.categoryId)}
              interactionDisabled={queryIsActive}
              onExpandedChange={(expanded) => {
                if (row.kind === "category") {
                  setCategoryExpanded(row.category.categoryId, expanded);
                }
              }}
            />
          )}
        />
      )}
    </SidebarTabPanel>
  );
}
