import { useSidebarDragStore } from "@/features/core/sidebarDrag";

/**
 * Overlay content for sidebar item drag (node-template).
 * Renders inside Workspace's DragOverlay when dragging from sidebar.
 */
export function SidebarDragOverlay() {
  const activeDrag = useSidebarDragStore((s) => s.activeDrag);
  if (!activeDrag) return null;
  return (
    <div className="bg-white/80 border border-blue-500 rounded px-3 py-1.5 shadow-xl flex items-center gap-2 cursor-grabbing">
      <div className="w-2 h-2 rounded-full bg-blue-500" />
      <span className="text-xs font-bold text-gray-700">
        {activeDrag.template?.title || activeDrag.template?.nodeType}
      </span>
    </div>
  );
}
