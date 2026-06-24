import { useRef, useEffect } from "react";
import { getViewport, subscribeToViewport } from "@/features/core/viewport";

export const TransformContainer = ({ graphId, children }: { graphId: string; children: React.ReactNode }) => {
    const containerRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (!graphId) return;
        return subscribeToViewport(graphId, (canvas) => {
            const el = containerRef.current;
            if (el) {
                el.style.transform = `translate3d(${canvas.x}px, ${canvas.y}px, 0) scale(${canvas.scale})`;
            }
        });
    }, [graphId]);

    const initial = graphId ? getViewport(graphId) : { x: 0, y: 0, scale: 1 };
    return (
        <div
            ref={containerRef}
            style={{
                transform: `translate3d(${initial.x}px, ${initial.y}px, 0) scale(${initial.scale})`,
                transformOrigin: "0 0",
                backfaceVisibility: "hidden",
                WebkitBackfaceVisibility: "hidden",
            }}
        >
            {children}
        </div>
    );
};
