import { useTranslation } from "react-i18next";
import { useNodeCatalogBrowser } from "@/features/application/nodeCatalog/useNodeCatalogBrowser";
import { nodeCatalogErrorText } from "@/features/application/nodeCatalog/nodeCatalogErrorPresentation";
import type { LocalizedCatalogBrowserRow } from "@/features/domain/nodeCatalog/localizedCatalogTree";
import { SidebarTabPanel, SidebarVirtualTree } from "@/modules/workbench/public";
import { SidebarCatalogTreeRow } from "./SidebarCatalogTreeRow";

const CATEGORY_ROW_HEIGHT = 28;
const ITEM_ROW_ESTIMATE = 32;

function rowEstimate(row: LocalizedCatalogBrowserRow | undefined): number {
  return row?.kind === "category" ? CATEGORY_ROW_HEIGHT : ITEM_ROW_ESTIMATE;
}

export function SidebarNodesTab() {
  const { t } = useTranslation();
  const { status, error, catalog, rows, expandedCategoryIds, setCategoryExpanded } =
    useNodeCatalogBrowser();
  return (
    <SidebarTabPanel>
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
          emptyMessage={t("sidebar.noNodes")}
          getRowKey={(row) => row.rowKey}
          getRowDepth={(row) => row.depth}
          estimateSize={rowEstimate}
          renderRow={(row) => (
            <SidebarCatalogTreeRow
              row={row}
              expanded={row.kind === "category" && expandedCategoryIds.has(row.category.categoryId)}
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
