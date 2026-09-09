import { VscSymbolMethod, VscSymbolProperty } from "react-icons/vsc";
import { useActivityPanelDocument } from "@/features/application/sidebar/useActivityPanelDocument";
import {
  ActivityPanelDocumentView,
  SidebarListItem,
  SIDEBAR_ROW_ICON_SIZE,
} from "@/modules/workbench/public";
import { DRAG_TYPES } from "@/features/core/dnd";
import type { NodeTemplateDragData } from "@/features/core/dnd";

export function SidebarNodesTab() {
  const query = useActivityPanelDocument("nodes");
  return (
    <ActivityPanelDocumentView
      panelId="nodes"
      document={query.document}
      error={query.error}
      expanded={query.expanded}
      onExpandedChange={query.setExpanded}
      onRetry={query.refresh}
      renderItem={(item, depth) => {
        if (item.kind !== "node") return null;
        const Icon = item.creation.kind === "resourceBound" ? VscSymbolMethod : VscSymbolProperty;
        return (
          <SidebarListItem
            id={`node-${item.key}`}
            indentDepth={depth}
            icon={<Icon size={SIDEBAR_ROW_ICON_SIZE} />}
            label={<span title={item.creation.nodeTypeId}>{item.title}</span>}
            dragData={
              {
                type: DRAG_TYPES.NODE_TEMPLATE,
                template: { title: item.title, descriptor: item.creation },
              } satisfies NodeTemplateDragData
            }
          />
        );
      }}
    />
  );
}
