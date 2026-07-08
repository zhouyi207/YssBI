import { useSidebarDragStore } from "@/features/core/sidebarDrag";
import { getSidebarDragOverlayLabel } from "@/features/core/dnd";

/**
 * Overlay content for sidebar item drag (node-template).
 * Renders inside Workspace's DragOverlay when dragging from sidebar.
 */
export function SidebarDragOverlay() {
  const activeDrag = useSidebarDragStore((s) => s.activeDrag);
  if (!activeDrag) return null;
  return (
    <div className="flex cursor-grabbing items-center gap-2 rounded-md border border-primary/60 bg-card/95 px-3 py-1.5 shadow-xl backdrop-blur-sm">
      <div className="h-2 w-2 rounded-full bg-primary" />
      <span className="text-xs font-bold text-foreground">
        {getSidebarDragOverlayLabel(activeDrag)}
      </span>
    </div>
  );
}
