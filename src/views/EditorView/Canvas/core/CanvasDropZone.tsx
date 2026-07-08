import { useDroppable } from "@dnd-kit/core";
import { DROP_TYPES, getCanvasDropZoneId } from "@/features/core/dnd";

/**
 * Canvas / watermark droppable target.
 * Visual preview is rendered by `EditorDropPreviewOverlay` via `useEditorDragPreviewMonitor`.
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
    data: { dropType: DROP_TYPES.CANVAS, groupId },
  });

  return (
    <div ref={setNodeRef} className="absolute inset-0 pointer-events-none">
      <div className="absolute inset-0 pointer-events-auto">
        {children}
      </div>
    </div>
  );
}
