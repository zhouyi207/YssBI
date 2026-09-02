import { memo } from "react";
import { useTranslation } from "react-i18next";
import { VscSymbolEvent, VscSymbolMethod } from "react-icons/vsc";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { openGraphInEditor } from "@/features/application/editor/openGraphInEditor";
import { buildSidebarDragData } from "@/features/application/sidebar";
import { revealDetails } from "@/features/application/editor/rightSidebarActions";
import { TYPE_ICON_COLORS } from "@/features/domain/sidebar";
import {
  SidebarListItem,
  SidebarRowActionButton,
  SIDEBAR_ROW_ICON_SIZE,
} from "@/modules/workbench/public";
import type { GraphResourceType } from "./projectSidebarTypes";

export const SidebarGraphRow = memo(function SidebarGraphRow({
  id,
  name,
  graphType,
  indentDepth = 0,
  isSelected = false,
  diagnosticCount = 0,
  onContextMenu,
}: {
  id: string;
  name: string;
  graphType: GraphResourceType;
  indentDepth?: number;
  isSelected?: boolean;
  diagnosticCount?: number;
  onContextMenu: (e: React.MouseEvent) => void;
}) {
  const { t } = useTranslation();
  const iconColor = graphType === "event" ? TYPE_ICON_COLORS.event : TYPE_ICON_COLORS.function;
  const icon =
    graphType === "event" ? (
      <VscSymbolEvent size={SIDEBAR_ROW_ICON_SIZE} style={{ color: iconColor }} />
    ) : (
      <VscSymbolMethod size={SIDEBAR_ROW_ICON_SIZE} style={{ color: iconColor }} />
    );

  return (
    <SidebarListItem
      id={id}
      dragData={buildSidebarDragData(id, name, graphType)}
      isSelected={isSelected}
      indentDepth={indentDepth}
      icon={icon}
      label={name}
      onClick={async (e) => {
        e.stopPropagation();
        const revealing = revealDetails({ kind: graphType, path: id });
        void openGraphInEditor(id, name, graphType, undefined, { pinned: false });
        await revealing;
      }}
      onDoubleClick={(e) => {
        e.stopPropagation();
        void openGraphInEditor(id, name, graphType);
      }}
      onContextMenu={onContextMenu}
      trailing={
        <>
          {diagnosticCount > 0 && (
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="inline-block h-1.5 w-1.5 shrink-0 rounded-full bg-amber-400" />
              </TooltipTrigger>
              <TooltipContent side="top">
                {t("graphDiagnostics.sidebarTooltip", { count: diagnosticCount })}
              </TooltipContent>
            </Tooltip>
          )}
          <SidebarRowActionButton
            isSelected={isSelected}
            tooltip={t("sidebar.open")}
            onClick={(e) => {
              e.stopPropagation();
              void openGraphInEditor(id, name, graphType);
            }}
          />
        </>
      }
    />
  );
});
