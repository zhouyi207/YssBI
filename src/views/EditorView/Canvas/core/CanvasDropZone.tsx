import { useDroppable } from "@dnd-kit/core";

export const CANVAS_DROP_ZONE_ID_PREFIX = "canvas-drop-zone-";

export function getCanvasDropZoneId(groupId: string) {
  return `${CANVAS_DROP_ZONE_ID_PREFIX}${groupId}`;
}

/**
 * Invisible droppable zone that covers the canvas area.
 * Allows sidebar items to be dropped onto the canvas.
 */
export function CanvasDropZone({
  groupId,
  children,
}: {
  groupId: string;
  children: React.ReactNode;
}) {
  const { setNodeRef } = useDroppable({
    id: getCanvasDropZoneId(groupId),
    data: { dropType: "canvas", groupId },
  });

  return (
    <div ref={setNodeRef} className="absolute inset-0 pointer-events-none">
      <div className="absolute inset-0 pointer-events-auto">{children}</div>
    </div>
  );
}
