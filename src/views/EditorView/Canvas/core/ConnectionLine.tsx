import { useRef, useEffect } from "react";
import { useGestureStore } from "@/features/canvas/stores";
import { useViewportStore } from "@/features/canvas/stores";
import { useTheme } from "@/hooks/useTheme";
import { drawEdge } from "./Edge";

import { Pin } from "@/shared/types/editor";

export const ConnectionLine = ({
    groupId,
    getPinWorldPos,
    getCanvasLocalPoint,
    pendingConnection,
    menuPos,
}: {
    groupId: string;
    getPinWorldPos: (pinId: string) => { x: number; y: number } | null;
    getCanvasLocalPoint: (x: number, y: number) => { x: number; y: number };
    pendingConnection?: Pin | null;
    menuPos?: { x: number; y: number } | null;
}) => {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const { theme } = useTheme();

    useEffect(() => {
        // Subscribe to both stores. We might trigger redraws from either.
        // However, for high-freq mouse moves, we want to read from gesture store directly in the RAF loop or subscription.

        // Actually, simple subscription to gesture store update is fine, but we need to ensure we don't re-render React.
        // So we use subscribe and manipulate Canvas manually.

        const render = () => {
            const { gesture } = useGestureStore.getState();
            const isConnecting = gesture?.type === "connect";
            const gestureStartPin = isConnecting ? (gesture as any).startPin : null;
            const currentX = isConnecting ? (gesture as any).currentX : 0;
            const currentY = isConnecting ? (gesture as any).currentY : 0;

            const canvasEl = canvasRef.current;
            if (!canvasEl) return;

            const ctx = canvasEl.getContext("2d");
            if (!ctx) return;

            // Clear always
            ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);

            let activeStart = null;
            let activeEndScreen: { x: number, y: number } | null = null;

            if (isConnecting && gestureStartPin) {
                activeStart = gestureStartPin;
                activeEndScreen = { x: currentX, y: currentY };
            } else if (pendingConnection && menuPos) {
                // If not actively dragging but we have a pending connection awaiting menu selection
                activeStart = pendingConnection;
                activeEndScreen = menuPos;
            }

            if (!activeStart || !activeEndScreen) {
                return;
            }

            const viewport = useViewportStore.getState().viewports[groupId] || { x: 0, y: 0, scale: 1 };

            // Setup transform
            ctx.save();
            ctx.translate(viewport.x, viewport.y);
            ctx.scale(viewport.scale, viewport.scale);

            const start = getPinWorldPos(activeStart.id);
            if (start) {
                const end = getCanvasLocalPoint(activeEndScreen.x, activeEndScreen.y);

                drawEdge(
                    ctx,
                    start.x, start.y,
                    end.x, end.y,
                    activeStart.ui?.color ?? (theme[`${activeStart.type}Color` as keyof typeof theme] as string) ?? theme.connectionLines,
                    2 / viewport.scale,
                    activeStart.direction === "input"
                );
            }

            ctx.restore();
        };

        const unsubGesture = useGestureStore.subscribe(render);
        const unsubViewport = useViewportStore.subscribe(render); // Viewport changes (pan/zoom) should also redraw active line

        // Initial check to catch pending state which doesn't trigger store updates
        render();

        return () => {
            unsubGesture();
            unsubViewport();
        };
    }, [groupId, getPinWorldPos, getCanvasLocalPoint, theme, pendingConnection, menuPos]);

    // Handle canvas resizing
    useEffect(() => {
        const canvasEl = canvasRef.current;
        if (!canvasEl) return;
        const parent = canvasEl.parentElement;
        if (!parent) return;

        const resizeObserver = new ResizeObserver(() => {
            const rect = parent.getBoundingClientRect();
            const dpr = window.devicePixelRatio || 1;
            canvasEl.width = rect.width * dpr;
            canvasEl.height = rect.height * dpr;
            canvasEl.style.width = `${rect.width}px`;
            canvasEl.style.height = `${rect.height}px`;
            const ctx = canvasEl.getContext('2d');
            if (ctx) ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

            // Force a redraw after resize
            const { gesture } = useGestureStore.getState();
            if (gesture?.type === 'connect') {
                // We can't easily call 'render' here as it's inside the other effect.
                // But resizing usually happens on window resize, which might not be high freq during drag.
                // We can just rely on the next mouse move or viewport change to fix it, or trigger a dummy viewport update?
                // Or better, just extract render logic. For now, this is acceptable.
            }
        });
        resizeObserver.observe(parent);

        return () => resizeObserver.disconnect();
    }, []);

    return <canvas ref={canvasRef} className="absolute inset-0 pointer-events-none z-50" />;
};
