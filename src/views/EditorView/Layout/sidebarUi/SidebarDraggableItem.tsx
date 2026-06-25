import { DRAG_TYPES } from "@/features/core/dnd";
import { useDraggable } from "@dnd-kit/core";

/**
 * Draggable sidebar row shell. PointerSensor activationConstraint (distance: 5)
 * keeps click / doubleClick separate from drag.
 */
export function SidebarDraggableItem({
  id,
  dragData,
  children,
  className,
  style,
  onClick,
  onDoubleClick,
  onContextMenu,
}: {
  id: string;
  dragData: { type: string; template?: unknown } | null;
  children: React.ReactNode;
  className?: string;
  style?: React.CSSProperties;
  onClick?: (e: React.MouseEvent) => void;
  onDoubleClick?: (e: React.MouseEvent) => void;
  onContextMenu?: (e: React.MouseEvent) => void;
}) {
  const canDrag = !!dragData;
  const { attributes, listeners, setNodeRef } = useDraggable({
    id: `sidebar-item-${id}`,
    data: dragData ?? { type: DRAG_TYPES.NODE_TEMPLATE, template: {} },
    disabled: !canDrag,
  });

  return (
    <div
      ref={setNodeRef}
      {...(canDrag ? listeners : {})}
      {...(canDrag ? attributes : {})}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
      className={`${className ?? ""} ${canDrag ? "cursor-grab active:cursor-grabbing" : ""}`}
      style={{
        ...style,
        opacity: 1,
        touchAction: canDrag ? "none" : undefined,
      }}
    >
      {children}
    </div>
  );
}
