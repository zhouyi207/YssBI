import React from "react";
import { useSelectionStore } from "../Store/useSelectionStore";

export const SelectionBox = ({ canvasRef }: { canvasRef: React.RefObject<HTMLDivElement | null> }) => {
    const { startX, startY, currentX, currentY, isVisible } = useSelectionStore();

    if (!isVisible || !canvasRef.current) return null;

    const rect = canvasRef.current.getBoundingClientRect();

    const x1 = Math.min(startX, currentX);
    const y1 = Math.min(startY, currentY);
    const x2 = Math.max(startX, currentX);
    const y2 = Math.max(startY, currentY);

    return (
        <div
            className="absolute border border-[var(--accent-color)] bg-[var(--selection-region)] pointer-events-none z-50"
            style={{
                left: x1 - rect.left,
                top: y1 - rect.top,
                width: x2 - x1,
                height: y2 - y1,
            }}
        />
    );
};
