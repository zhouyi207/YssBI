import React from "react";
import { useShallow } from "zustand/react/shallow";
import { useSelectionStore } from "@/features/core/canvas";

export const SelectionBox = ({ canvasRef }: { canvasRef: React.RefObject<HTMLDivElement | null> }) => {
    const { startX, startY, currentX, currentY, isVisible } = useSelectionStore(
        useShallow((s) => ({
            startX: s.startX,
            startY: s.startY,
            currentX: s.currentX,
            currentY: s.currentY,
            isVisible: s.isVisible,
        }))
    );

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
