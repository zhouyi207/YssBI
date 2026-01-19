// drag/DragLayer.tsx
import { useDrag } from "./DragContext";

export function DragLayer() {
  const { drag } = useDrag();

  if (!drag) return null;

  if (drag.type !== "node-template") return null;

  const isVariable = drag.template.category === "Variable";

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
      {isVariable ? (
        <div className="flex items-center gap-2 px-3 py-2 bg-blue-600 text-white rounded shadow-lg opacity-90 border border-blue-400">
          <div className="w-2 h-2 rounded-full bg-white" />
          <span className="text-xs font-bold">{drag.template.variableName}</span>
          <span className="text-[9px] bg-blue-500 px-1 rounded uppercase font-black">
            {drag.template.variableType}
          </span>
        </div>
      ) : (
        <div
          className="
            px-3 py-2
            bg-white border rounded shadow
            opacity-80
          "
        >
          {drag.template.title}
        </div>
      )}
    </div>
  );
}
