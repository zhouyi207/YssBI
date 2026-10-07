import { useDraggable, type Data } from "@dnd-kit/core";

/**
 * Draggable sidebar row shell. PointerSensor activationConstraint (distance: 5)
 * keeps clicks separate from drag.
 */
export function SidebarDraggableItem({
  id,
  dragData,
  children,
  className,
  style,
  onClick,
  onContextMenu,
  dragDisabledReason,
  onDisabledDragAttempt,
}: {
  id: string;
  dragData: Data | null;
  children: React.ReactNode;
  className?: string;
  style?: React.CSSProperties;
  onClick?: (e: React.MouseEvent) => void;
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
      onContextMenu={onContextMenu}
      title={!canDrag ? dragDisabledReason : undefined}
      className={`${className ?? ""} ${canDrag || onClick ? "cursor-pointer" : ""}`}
      style={{
        ...style,
        touchAction: canDrag ? "none" : undefined,
      }}
    >
      {children}
    </div>
  );
}
