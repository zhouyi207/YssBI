import { memo } from "react";
import { useTranslation } from "react-i18next";
import {
  VscSymbolEvent,
  VscSymbolMethod,
  VscGraphLine,
  VscTypeHierarchy,
  VscFileText,
} from "react-icons/vsc";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { openFileInEditor } from "@/features/application/editor/openFileInEditor";
import { useGraphRead } from "@/features/core/graph/read";
import { buildSidebarDragData } from "@/features/application/sidebar";
import { TYPE_ICON_COLORS } from "./resourceIconColors";
import {
  SidebarListItem,
  SidebarRowActionButton,
  SIDEBAR_ROW_ICON_SIZE,
} from "@/modules/workbench/public";
import type { FileResourceKind } from "@/shared/types/domain/resource";
import type { FileResourceRef } from "@/features/application/resource/resourceActions";

export const SidebarFileRow = memo(function SidebarFileRow({
  id,
  name,
  kind,
  indentDepth = 0,
  isSelected = false,
  onContextMenu,
  onOpen,
}: {
  id: string;
  name: string;
  kind: FileResourceKind;
  indentDepth?: number;
  isSelected?: boolean;
  onContextMenu: (e: React.MouseEvent) => void;
  onOpen?: (ref: FileResourceRef) => void;
}) {
  const { t } = useTranslation();
  const diagnosticCount = useGraphRead((snapshot) =>
    kind === "event_graph" || kind === "function_graph"
      ? (snapshot.graphEntities[id]?.diagnostics.length ?? 0)
      : 0,
  );
  const Icon = {
    event_graph: VscSymbolEvent,
    function_graph: VscSymbolMethod,
    chart: VscGraphLine,
    mind: VscTypeHierarchy,
    doc: VscFileText,
  }[kind];
  const dragBuilders: Partial<
    Record<FileResourceKind, () => ReturnType<typeof buildSidebarDragData>>
  > = {
    event_graph: () => buildSidebarDragData(id, name, "event_graph"),
    function_graph: () => buildSidebarDragData(id, name, "function_graph"),
  };
  const open = () => (onOpen ? onOpen({ id, kind }) : void openFileInEditor(id, kind));
  const icon = <Icon size={SIDEBAR_ROW_ICON_SIZE} style={{ color: TYPE_ICON_COLORS[kind] }} />;

  return (
    <SidebarListItem
      id={id}
      dragData={dragBuilders[kind]?.()}
      isSelected={isSelected}
      indentDepth={indentDepth}
      icon={icon}
      label={name}
      onClick={(e) => {
        e.stopPropagation();
        open();
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
              open();
            }}
          />
        </>
      }
    />
  );
});
