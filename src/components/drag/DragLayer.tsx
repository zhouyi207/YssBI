// drag/DragLayer.tsx
import { useDrag } from "./DragContext";

export function DragLayer() {
  const { drag } = useDrag();

  if (!drag) return null;

  if (drag.type !== "node-template") return null;

  return (
    <div
      style={{
        position: "fixed",
        left: drag.x,
        top: drag.y,
        transform: "translate(-50%, -50%)",
        pointerEvents: "none",
        zIndex: 9999,
      }}
    >
      <div
        className="
          px-3 py-2
          bg-white border rounded shadow
          opacity-80
        "
      >
        {drag.template.title}
      </div>
    </div>
  );
}
