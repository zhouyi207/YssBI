import { VscAdd } from "react-icons/vsc";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import type { ProjectResourceBrowserRow } from "@/features/application/sidebar/projectResourceBrowser";
import type { DetailTarget } from "@/features/core/editor/detail/detailTypes";
import { PROJECT_TREE_CATEGORY_IDS } from "@/features/core/sidebar/projectTreeState";
import type { ProjectTreeCategoryId } from "@/features/core/sidebar/projectTreeState";
import {
  SIDEBAR_ROW_ICON_SIZE,
  SidebarSectionEmptyState,
  SidebarTreeCategoryRow,
} from "@/modules/workbench/public";
import type { GraphResourceType } from "./projectSidebarTypes";
import { SidebarGraphRow } from "./SidebarGraphRow";
import { SidebarChartRow } from "./SidebarChartRow";
import { SidebarDataRow } from "./SidebarDataRow";

export interface SidebarProjectTreeActions {
  onAddEvent: () => void;
  onAddFunction: () => void;
  onAddChart: () => void;
  onImportData: () => void;
  onCategoryContextMenu: (event: React.MouseEvent, categoryId: ProjectTreeCategoryId) => void;
  onGraphContextMenu: (
    event: React.MouseEvent,
    target: { type: "graph"; id: string; name: string; graphType: GraphResourceType },
  ) => void;
  onChartContextMenu: (event: React.MouseEvent, path: string, name: string) => void;
  onOpenChart: (path: string, name: string) => void;
  onDatabaseContextMenu: (event: React.MouseEvent, id: string, name: string) => void;
}

function categoryAddConfig(
  categoryId: ProjectTreeCategoryId,
  actions: SidebarProjectTreeActions,
  t: ReturnType<typeof useTranslation>["t"],
): { onAdd: () => void; ariaLabel: string } | null {
  switch (categoryId) {
    case PROJECT_TREE_CATEGORY_IDS.events:
      return { onAdd: actions.onAddEvent, ariaLabel: t("canvas.newEventGraph") };
    case PROJECT_TREE_CATEGORY_IDS.functions:
      return { onAdd: actions.onAddFunction, ariaLabel: t("canvas.newFunctionGraph") };
    case PROJECT_TREE_CATEGORY_IDS.charts:
      return {
        onAdd: actions.onAddChart,
        ariaLabel: t("contextMenu.sidebar.newChart"),
      };
    case PROJECT_TREE_CATEGORY_IDS.data:
      return { onAdd: actions.onImportData, ariaLabel: t("contextMenu.sidebar.importData") };
  }
}

export function SidebarProjectTreeRow({
  row,
  actions,
  detailTarget,
  graphDiagnosticCounts,
  onCategoryExpandedChange,
}: {
  row: ProjectResourceBrowserRow;
  actions: SidebarProjectTreeActions;
  detailTarget: DetailTarget | null;
  graphDiagnosticCounts: Record<string, number>;
  onCategoryExpandedChange: (categoryId: ProjectTreeCategoryId, expanded: boolean) => void;
}) {
  const { t } = useTranslation();

  switch (row.kind) {
    case "category": {
      const addConfig = categoryAddConfig(row.categoryId, actions, t);
      return (
        <SidebarTreeCategoryRow
          categoryId={row.categoryId}
          label={row.label}
          depth={row.level}
          expanded={row.expanded}
          onExpandedChange={(expanded) => onCategoryExpandedChange(row.categoryId, expanded)}
          onContextMenu={(event) => actions.onCategoryContextMenu(event, row.categoryId)}
          trailing={
            addConfig ? (
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                aria-label={addConfig.ariaLabel}
                onClick={(event) => {
                  event.stopPropagation();
                  addConfig.onAdd();
                }}
                className="size-6 shrink-0 p-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100"
              >
                <VscAdd size={SIDEBAR_ROW_ICON_SIZE} />
              </Button>
            ) : undefined
          }
        />
      );
    }
    case "empty": {
      return (
        <SidebarSectionEmptyState
          level={row.level}
          message={row.message}
          onContextMenu={(event) => actions.onCategoryContextMenu(event, row.categoryId)}
        />
      );
    }
    case "graph":
      return (
        <SidebarGraphRow
          id={row.id}
          name={row.name}
          graphType={row.graphType}
          indentDepth={row.level}
          isSelected={detailTarget?.kind === row.graphType && detailTarget.path === row.id}
          diagnosticCount={graphDiagnosticCounts[row.id] ?? 0}
          onContextMenu={(event) =>
            actions.onGraphContextMenu(event, {
              type: "graph",
              id: row.id,
              name: row.name,
              graphType: row.graphType,
            })
          }
        />
      );
    case "chart":
      return (
        <SidebarChartRow
          chartPath={row.chartPath}
          name={row.name}
          indentDepth={row.level}
          isSelected={detailTarget?.kind === "chart" && detailTarget.chartPath === row.chartPath}
          onOpen={actions.onOpenChart}
          onContextMenu={(event) => actions.onChartContextMenu(event, row.chartPath, row.name)}
        />
      );
    case "database":
      return (
        <SidebarDataRow
          id={row.id}
          resourcePath={row.resourcePath}
          name={row.name}
          data={row.data}
          indentDepth={row.level}
          isSelected={detailTarget?.kind === "data" && detailTarget.id === row.id}
          onContextMenu={(event) => actions.onDatabaseContextMenu(event, row.id, row.name)}
        />
      );
    default: {
      const _exhaustive: never = row;
      return _exhaustive;
    }
  }
}
