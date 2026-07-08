import { useDroppable } from "@dnd-kit/core";
import { useSidebarDragStore } from "@/features/core/sidebarDrag";
import { DROP_TYPES, getCanvasDropZoneId, getSidebarResourceFromDragState } from "@/features/core/dnd";

/**
 * Invisible droppable zone that covers the canvas area.
 * Node templates are inserted into the canvas; graph resources are opened as tabs.
 */
export function CanvasDropZone({
  groupId,
  children,
}: {
  groupId: string;
  children: React.ReactNode;
}) {
  const { setNodeRef, isOver } = useDroppable({
    id: getCanvasDropZoneId(groupId),
    data: { dropType: DROP_TYPES.CANVAS, groupId },
  });
  const sidebarResource = useSidebarDragStore((s) => getSidebarResourceFromDragState(s.activeDrag));

  return (
    <div ref={setNodeRef} className="absolute inset-0 pointer-events-none">
      <div className="absolute inset-0 pointer-events-auto">
        {children}
        {sidebarResource && isOver && (
          <div className="absolute inset-0 z-30 bg-blue-500/12 ring-2 ring-inset ring-blue-400/60 pointer-events-none">
            <div className="absolute inset-3 border border-blue-300/60 bg-blue-500/10 flex items-center justify-center">
              <div className="rounded bg-[var(--workbench-bg)]/90 border border-blue-300/50 px-3 py-1.5 text-[12px] font-medium text-blue-100 shadow-lg">
                Open {sidebarResource.name}
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
