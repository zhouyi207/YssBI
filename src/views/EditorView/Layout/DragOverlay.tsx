// Components/DragOverlayLayer.tsx
import { DragOverlay } from "@dnd-kit/core";
import { useDragContext } from "../Context/DragProvider";

export const DragLayer: React.FC = () => {
  const { activeDrag } = useDragContext();

  return (
    <DragOverlay>
      {activeDrag ? (
        <div className="fixed inset-0 pointer-events-none z-[9999]">
          <div
            className="absolute bg-white/80 border border-blue-500 rounded px-3 py-1.5 shadow-xl flex items-center gap-2"
          >
            <div className="w-2 h-2 rounded-full bg-blue-500" />
            <span className="text-xs font-bold text-gray-700">
              {activeDrag.template.title || activeDrag.template.nodeType}
            </span>
          </div>
        </div>
      ) : null}
    </DragOverlay>
  );
};
