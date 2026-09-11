import { useGraphDiagnosticCounts } from "@/features/application/graphDiagnostics/useGraphDiagnosticCounts";
import { useActivityPanelDocument } from "@/features/application/sidebar/useActivityPanelDocument";
import { useActiveProjectGraph } from "@/features/application/sidebar/useActiveProjectGraph";
import { useDetailTarget } from "@/features/application/editor";
import { ActivityPanelDocumentView } from "@/modules/workbench/public";
import {
  PROJECT_TREE_CATEGORY_IDS,
  type ProjectTreeCategoryId,
} from "@/features/core/sidebar/projectTreeState";
import { SidebarProjectTreeRow, type SidebarProjectTreeActions } from "./SidebarProjectTreeRow";

export function SidebarProjectTab({ actions }: { actions: SidebarProjectTreeActions }) {
  const query = useActivityPanelDocument("project");
  const detailTarget = useDetailTarget();
  const activeGraph = useActiveProjectGraph();
  const graphDiagnosticCounts = useGraphDiagnosticCounts();
  return (
    <ActivityPanelDocumentView
      panelId="project"
      document={query.document}
      error={query.error}
      onRetry={query.refresh}
      expanded={query.expanded}
      onExpandedChange={query.setExpanded}
      actions={{
        newEvent: actions.onAddEvent,
        newFunction: actions.onAddFunction,
        newChart: actions.onAddChart,
        importData: actions.onImportData,
      }}
      onContextMenu={(event, row) => {
        if (Object.values(PROJECT_TREE_CATEGORY_IDS).includes(row.id as ProjectTreeCategoryId))
          actions.onCategoryContextMenu(event, row.id as ProjectTreeCategoryId);
      }}
      renderItem={(item, depth) => (
        <SidebarProjectTreeRow
          item={item}
          depth={depth}
          actions={actions}
          detailTarget={detailTarget}
          activeGraph={activeGraph}
          graphDiagnosticCounts={graphDiagnosticCounts}
        />
      )}
    />
  );
}
