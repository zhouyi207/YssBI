import { VscSymbolMethod, VscSymbolProperty } from "react-icons/vsc";
import { useTranslation } from "react-i18next";
import { useActivityPanelDocument } from "@/features/application/sidebar/useActivityPanelDocument";
import {
  ActivityPanelDocumentView,
  SidebarListItem,
  SIDEBAR_ROW_ICON_SIZE,
} from "@/modules/workbench/public";
import { DRAG_TYPES } from "@/features/core/dnd";
import type { NodeTemplateDragData } from "@/features/core/dnd";

export function SidebarNodesTab() {
  const { t } = useTranslation();
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
            icon={
              <Icon
                size={SIDEBAR_ROW_ICON_SIZE}
                className={!item.available ? "opacity-50" : undefined}
              />
            }
            label={
              <span
                className={!item.available ? "opacity-50" : undefined}
                title={item.creation.nodeTypeId}
              >
                {item.title}
              </span>
            }
            trailing={
              !item.available ? (
                <span className="shrink-0 text-[10px] text-muted-foreground">
                  {t("canvas.nodePalette.unavailable")}
                </span>
              ) : undefined
            }
            dragDisabledReason={!item.available ? t("canvas.nodePalette.unavailable") : undefined}
            dragData={
              item.available
                ? ({
                    type: DRAG_TYPES.NODE_TEMPLATE,
                    template: { title: item.title, descriptor: item.creation },
                  } satisfies NodeTemplateDragData)
                : null
            }
          />
        );
      }}
    />
  );
}
