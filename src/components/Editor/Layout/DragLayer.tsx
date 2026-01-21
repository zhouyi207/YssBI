import React from "react";
import { useDrag } from "../Context/DragContext";

export const DragLayer: React.FC = () => {
    const { drag } = useDrag();

    if (!drag) return null;

    return (
        <div
            className="fixed inset-0 pointer-events-none z-[9999]"
            style={{ isolation: "isolate" }}
        >
            <div
                className="absolute bg-white/80 border border-blue-500 rounded px-3 py-1.5 shadow-xl flex items-center gap-2"
                style={{
                    left: drag.x,
                    top: drag.y,
                    transform: "translate(10px, 10px)",
                }}
            >
                <div className="w-2 h-2 rounded-full bg-blue-500" />
                <span className="text-xs font-bold text-gray-700">
                    {drag.template.title || drag.template.type}
                </span>
            </div>
        </div>
    );
};
