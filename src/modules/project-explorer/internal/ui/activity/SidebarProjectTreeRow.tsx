import type { ActivityItem } from "@/shared/types/domain/activityPanel";
import type { DetailTarget } from "@/features/core/editor/detail/detailTypes";
import type { ActiveProjectGraph } from "@/features/application/sidebar/useActiveProjectGraph";
import type { ProjectTreeCategoryId } from "@/features/core/sidebar/projectTreeState";
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

export function SidebarProjectTreeRow({
  item,
  depth,
  actions,
  detailTarget,
  activeGraph,
  graphDiagnosticCounts,
}: {
  item: ActivityItem;
  depth: number;
  actions: SidebarProjectTreeActions;
  detailTarget: DetailTarget | null;
  activeGraph: Pick<ActiveProjectGraph, "path" | "kind"> | null;
  graphDiagnosticCounts: Record<string, number>;
}) {
  switch (item.kind) {
    case "graph":
      return (
        <SidebarGraphRow
          id={item.path}
          name={item.name}
          graphType={item.graphType}
          indentDepth={depth}
          isSelected={activeGraph?.kind === item.graphType && activeGraph.path === item.path}
          diagnosticCount={graphDiagnosticCounts[item.path] ?? 0}
          onContextMenu={(event) =>
            actions.onGraphContextMenu(event, {
              type: "graph",
              id: item.path,
              name: item.name,
              graphType: item.graphType,
            })
          }
        />
      );
    case "chart":
      return (
        <SidebarChartRow
          chartPath={item.path}
          name={item.name}
          indentDepth={depth}
          isSelected={detailTarget?.kind === "chart" && detailTarget.chartPath === item.path}
          onOpen={actions.onOpenChart}
          onContextMenu={(event) => actions.onChartContextMenu(event, item.path, item.name)}
        />
      );
    case "database":
      return (
        <SidebarDataRow
          id={item.id}
          name={item.name}
          resourcePath={item.resourcePath}
          indentDepth={depth}
          isSelected={detailTarget?.kind === "data" && detailTarget.id === item.id}
          onContextMenu={(event) => actions.onDatabaseContextMenu(event, item.id, item.name)}
        />
      );
    default:
      return null;
  }
}
