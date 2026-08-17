import { useDroppable } from "@dnd-kit/core";
import { DROP_TYPES, getCanvasDropZoneId } from "@/features/core/dnd";

/**
 * Canvas / watermark droppable target.
 * Visual preview is rendered by `EditorDropPreviewOverlay` via `useEditorDragPreviewMonitor`.
 */
export function CanvasDropZone({
  groupId,
  mode,
  children,
}: {
  groupId: string;
  mode: 'interactive' | 'preview';
  children: React.ReactNode;
}) {
  const interactive = mode === 'interactive';
  const { setNodeRef } = useDroppable({
    id: getCanvasDropZoneId(groupId),
    data: { dropType: DROP_TYPES.CANVAS, groupId },
    disabled: !interactive,
  });

  return (
    <div ref={setNodeRef} className="absolute inset-0 pointer-events-none">
      <div className={`absolute inset-0 ${interactive ? 'pointer-events-auto' : ''}`}>
        {children}
      </div>
    </div>
  );
}
