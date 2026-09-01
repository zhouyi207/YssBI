import type { SidebarDragPayload } from "@/features/core/dnd";
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
  dragDisabledReason,
  onDisabledDragAttempt,
}: {
  id: string;
  dragData: SidebarDragPayload | null;
  children: React.ReactNode;
  className?: string;
  style?: React.CSSProperties;
  onClick?: (e: React.MouseEvent) => void;
  onDoubleClick?: (e: React.MouseEvent) => void;
  onContextMenu?: (e: React.MouseEvent) => void;
  dragDisabledReason?: string;
  onDisabledDragAttempt?: () => void;
}) {
  const canDrag = !!dragData;
  const { attributes, listeners, setNodeRef } = useDraggable({
    id: `sidebar-item-${id}`,
    data: dragData ?? {},
    disabled: !canDrag,
  });

  return (
    <div
      ref={setNodeRef}
      {...(canDrag ? listeners : {})}
      {...(canDrag ? attributes : {})}
      {...(!canDrag && dragDisabledReason ? { onPointerDown: onDisabledDragAttempt } : {})}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      onContextMenu={onContextMenu}
      aria-disabled={!canDrag && Boolean(dragDisabledReason)}
      title={!canDrag ? dragDisabledReason : undefined}
      className={`${className ?? ""} ${canDrag ? "cursor-pointer" : ""}`}
      style={{
        ...style,
        opacity: !canDrag && dragDisabledReason ? 0.65 : 1,
        touchAction: canDrag ? "none" : undefined,
      }}
    >
      {children}
    </div>
  );
}
