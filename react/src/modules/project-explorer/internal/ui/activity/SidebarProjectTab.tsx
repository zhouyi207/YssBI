import { memo } from "react";
import { isActivityFileItem } from "@/shared/types/domain/activityPanel";
import { useActivityPanelDocument } from "@/features/application/sidebar/useActivityPanelDocument";
import { useActiveProjectResource } from "@/features/application/sidebar/useActiveProjectResource";
import { ActivityPanelDocumentView } from "@/modules/workbench/public";
import {
  PROJECT_TREE_CATEGORY_IDS,
  type ProjectTreeCategoryId,
} from "@/features/core/sidebar/projectTreeState";
import { SidebarProjectTreeRow, type SidebarProjectTreeActions } from "./SidebarProjectTreeRow";

export const SidebarProjectTab = memo(function SidebarProjectTab({
  actions,
}: {
  actions: SidebarProjectTreeActions;
}) {
  const query = useActivityPanelDocument("project");
  const activeResource = useActiveProjectResource();
  return (
    <ActivityPanelDocumentView
      panelId="project"
      document={query.document}
      error={query.error}
      onRetry={query.refresh}
      expanded={query.expanded}
      onExpandedChange={query.setExpanded}
      actions={{
        newEventGraph: () => actions.onCreateFile("event_graph"),
        newFunctionGraph: () => actions.onCreateFile("function_graph"),
        newChart: () => actions.onCreateFile("chart"),
        newMind: () => actions.onCreateFile("mind"),
        newDoc: () => actions.onCreateFile("doc"),
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
          isSelected={
            activeResource?.kind === item.kind &&
            activeResource.id ===
              (isActivityFileItem(item) ? item.path : item.kind === "database" ? item.id : null)
          }
        />
      )}
    />
  );
});
