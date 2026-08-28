import { useDroppable } from "@dnd-kit/core";
import { DROP_TYPES, getCanvasDropZoneId } from "@/features/application/viewCapabilities";

/**
 * Canvas / watermark droppable target.
 * Visual preview is rendered by `EditorDropPreviewOverlay` via `useEditorDragPreviewMonitor`.
 */
export function CanvasDropZone({
  panelInstanceId,
  groupId,
  graphPath,
  graphKind,
  mode,
  children,
}: {
  panelInstanceId: string;
  groupId: string;
  graphPath: string;
  graphKind: 'event' | 'function';
  mode: 'interactive' | 'preview';
  children: React.ReactNode;
}) {
  const interactive = mode === 'interactive';
  const { setNodeRef } = useDroppable({
    id: getCanvasDropZoneId(panelInstanceId),
    data: { dropType: DROP_TYPES.CANVAS, panelInstanceId, groupId, graphPath, graphKind },
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
