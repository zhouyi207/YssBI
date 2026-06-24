import { useRef, useEffect } from "react";
import { getViewport, subscribeToViewport } from "@/features/core/viewport";
import { GRID } from "@/app/appConfig/default";

export const ViewportGrid = ({ graphId }: { graphId: string }) => {
    const gridRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (!graphId) return;
        return subscribeToViewport(graphId, (canvas) => {
            const el = gridRef.current;
            if (el) {
                el.style.backgroundSize = `${GRID * canvas.scale}px ${GRID * canvas.scale}px`;
                el.style.backgroundPosition = `${canvas.x}px ${canvas.y}px`;
            }
        });
    }, [graphId]);

    const initial = graphId ? getViewport(graphId) : { x: 0, y: 0, scale: 1 };
    return (
        <div
            ref={gridRef}
            className="absolute inset-0 pointer-events-none"
            style={{
                backgroundImage: `
          linear-gradient(var(--grid-lines) 1px, transparent 1px),
          linear-gradient(90deg, var(--grid-lines) 1px, transparent 1px)
        `,
                backgroundSize: `${GRID * initial.scale}px ${GRID * initial.scale}px`,
                backgroundPosition: `${initial.x}px ${initial.y}px`,
            }}
        />
    );
};
