import type { Data } from "@dnd-kit/core";
import { SidebarDraggableItem } from "./SidebarDraggableItem";
import {
  sidebarItemIndent,
  sidebarItemLabelClass,
  sidebarItemRowClass,
  SIDEBAR_ROW_LEADING_SLOT_CLASS,
} from "./sidebarStyles";

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
  dragDisabledReason,
  onDisabledDragAttempt,
}: {
  id: string;
  dragData?: Data | null;
  isSelected?: boolean;
  indentDepth?: number;
  icon: React.ReactNode;
  label: React.ReactNode;
  trailing?: React.ReactNode;
  onClick?: (e: React.MouseEvent) => void;
  onDoubleClick?: (e: React.MouseEvent) => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  dragDisabledReason?: string;
  onDisabledDragAttempt?: () => void;
}) {
  return (
    <SidebarDraggableItem
      id={id}
      dragData={dragData}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
      dragDisabledReason={dragDisabledReason}
      onDisabledDragAttempt={onDisabledDragAttempt}
      className={sidebarItemRowClass(isSelected)}
      style={sidebarItemIndent(indentDepth)}
    >
      <span className={SIDEBAR_ROW_LEADING_SLOT_CLASS}>{icon}</span>
      <span className={sidebarItemLabelClass(isSelected)}>{label}</span>
      {trailing}
    </SidebarDraggableItem>
  );
}
