import type { SidebarDragPayload } from "@/features/core/dnd";
import { SidebarDraggableItem } from "./SidebarDraggableItem";
import { sidebarItemIndent, sidebarItemLabelClass, sidebarItemRowClass } from "./sidebarStyles";

export function SidebarListItem({
  id,
  dragData = null,
  isSelected = false,
  indentDepth = 0,
  icon,
  label,
  trailing,
  onClick,
  onDoubleClick,
  onContextMenu,
}: {
  id: string;
  dragData?: SidebarDragPayload | null;
  isSelected?: boolean;
  indentDepth?: number;
  icon: React.ReactNode;
  label: React.ReactNode;
  trailing?: React.ReactNode;
  onClick?: (e: React.MouseEvent) => void;
  onDoubleClick?: (e: React.MouseEvent) => void;
  onContextMenu?: (e: React.MouseEvent) => void;
}) {
  return (
    <SidebarDraggableItem
      id={id}
      dragData={dragData}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
      className={sidebarItemRowClass(isSelected)}
      style={sidebarItemIndent(indentDepth)}
    >
      <span className="flex shrink-0 items-center justify-center">{icon}</span>
      <span className={sidebarItemLabelClass()}>{label}</span>
      {trailing}
    </SidebarDraggableItem>
  );
}
