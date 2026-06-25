import { useId } from "react";
import { VscAdd, VscChevronDown } from "react-icons/vsc";
import { useDroppable } from "@dnd-kit/core";
import { Button } from "@/components/ui/button";
import { OverlayScrollbar } from "@/shared/ui/OverlayScrollbar";
import { DROP_TYPES } from "@/features/core/dnd";
import type { GraphResourceType } from "../sidebarContextMenu/sidebarContextMenuTypes";
import {
  sidebarCollapsibleHeaderClass,
  sidebarItemIndent,
  sidebarSectionLabelClass,
} from "./sidebarStyles";

export interface GraphFolderDropTarget {
  graphType: GraphResourceType;
  folderPath: string;
}

type SidebarCollapsibleSectionProps = {
  variant?: "stacked" | "nested";
  label: string;
  expanded: boolean;
  onToggle: () => void;
  onAdd?: () => void;
  indentDepth?: number;
  isActive?: boolean;
  leading?: React.ReactNode;
  dropTarget?: GraphFolderDropTarget;
  onHeaderContextMenu?: (e: React.MouseEvent) => void;
  onContentContextMenu?: (e: React.MouseEvent) => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  children: React.ReactNode;
};

export function SidebarCollapsibleSection({
  variant = "nested",
  label,
  expanded,
  onToggle,
  onAdd,
  indentDepth = 0,
  isActive = false,
  leading,
  dropTarget,
  onHeaderContextMenu,
  onContentContextMenu,
  onContextMenu,
  children,
}: SidebarCollapsibleSectionProps) {
  const fallbackDropId = useId();
  const headerContextMenu = onHeaderContextMenu ?? onContextMenu;
  const chevronSize = variant === "stacked" ? 12 : 11;

  const { setNodeRef, isOver } = useDroppable({
    id: dropTarget
      ? `graph-folder-drop-${dropTarget.graphType}-${dropTarget.folderPath || "root"}`
      : `graph-folder-drop-disabled-${fallbackDropId}`,
    data: dropTarget
      ? { dropType: DROP_TYPES.GRAPH_FOLDER, graphType: dropTarget.graphType, folderPath: dropTarget.folderPath }
      : undefined,
    disabled: !dropTarget,
  });

  const header = (
    <div
      role="button"
      tabIndex={0}
      onClick={(e) => {
        if ((e.target as HTMLElement).closest("[data-add-btn]")) return;
        e.stopPropagation();
        onToggle();
      }}
      onKeyDown={(e) => {
        if (e.key !== "Enter" && e.key !== " ") return;
        e.preventDefault();
        onToggle();
      }}
      onContextMenu={headerContextMenu}
      className={sidebarCollapsibleHeaderClass(isActive)}
      style={sidebarItemIndent(indentDepth)}
    >
      <span
        className="shrink-0 text-muted-foreground transition-transform duration-150 ease-out"
        style={{ transform: expanded ? "rotate(0deg)" : "rotate(-90deg)" }}
      >
        <VscChevronDown size={chevronSize} />
      </span>
      {leading ? <span className="flex shrink-0 items-center justify-center">{leading}</span> : null}
      <span className={sidebarSectionLabelClass()}>{label}</span>
      {onAdd ? (
        <Button
          data-add-btn
          type="button"
          variant="ghost"
          size="icon-xs"
          onClick={(e) => {
            e.stopPropagation();
            onAdd();
          }}
          className="shrink-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100"
        >
          <VscAdd size={11} />
        </Button>
      ) : null}
    </div>
  );

  const body = (
    <div
      className="grid overflow-hidden transition-[grid-template-rows] duration-150 ease-out"
      style={{ gridTemplateRows: expanded ? "1fr" : "0fr" }}
    >
      <div className="min-h-0">{children}</div>
    </div>
  );

  if (variant === "stacked") {
    return (
      <div
        ref={setNodeRef}
        className={`flex min-h-0 shrink-0 flex-col ${expanded ? "flex-1" : "flex-none"} ${
          isOver ? "bg-[var(--sidebar-hover)]" : ""
        }`}
        style={expanded ? { minHeight: 0 } : undefined}
      >
        {header}
        <div
          className="grid overflow-hidden transition-[grid-template-rows] duration-150 ease-out"
          style={{ gridTemplateRows: expanded ? "1fr" : "0fr" }}
        >
          <OverlayScrollbar className="min-h-0 flex-1">
            <div className="min-h-full" onContextMenu={onContentContextMenu}>
              {children}
            </div>
          </OverlayScrollbar>
        </div>
      </div>
    );
  }

  return (
    <div ref={setNodeRef} className={isOver ? "bg-[var(--sidebar-hover)]" : undefined}>
      {header}
      {body}
    </div>
  );
}
