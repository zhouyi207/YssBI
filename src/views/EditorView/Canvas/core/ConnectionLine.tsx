import { useRef, useEffect } from "react";
import { useGestureStore } from '@/features/core/gesture';
import { useViewportStore } from '@/features/core/viewport';
import { useTheme } from "@/features/core/theme/useTheme";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { drawEdge } from "./Edge";

import { Pin } from "@/shared/types/domain";

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

    // 用 ref 存储高频变化的值，避免 effect 因依赖变化而反复重建订阅
    const getPinWorldPosRef = useRef(getPinWorldPos);
    const getCanvasLocalPointRef = useRef(getCanvasLocalPoint);
    const themeRef = useRef(theme);
    const pendingConnectionRef = useRef(pendingConnection);
    const menuPosRef = useRef(menuPos);
    const renderRef = useRef<() => void>(() => {});

    useEffect(() => { getPinWorldPosRef.current = getPinWorldPos; }, [getPinWorldPos]);
    useEffect(() => { getCanvasLocalPointRef.current = getCanvasLocalPoint; }, [getCanvasLocalPoint]);
    useEffect(() => { themeRef.current = theme; }, [theme]);
    useEffect(() => { pendingConnectionRef.current = pendingConnection; }, [pendingConnection]);
    useEffect(() => { menuPosRef.current = menuPos; }, [menuPos]);

    useEffect(() => {
        renderRef.current();
    }, [pendingConnection, menuPos]);

    useEffect(() => {
        const render = () => {
            const { gesture } = useGestureStore.getState();
            const isConnecting = gesture?.type === "connect";
            const gestureStartPin = isConnecting ? (gesture as any).startPin : null;

            const canvasEl = canvasRef.current;
            if (!canvasEl) return;
            const ctx = canvasEl.getContext("2d");
            if (!ctx) return;

            ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);

            let activeStart = null;
            let endWorld: { x: number, y: number } | null = null;

            if (isConnecting && gestureStartPin) {
                activeStart = gestureStartPin;
                // 优先使用世界坐标（多 editor 同步正确），回退到屏幕坐标转换
                if ((gesture as any).worldX != null && (gesture as any).worldY != null) {
                    endWorld = { x: (gesture as any).worldX, y: (gesture as any).worldY };
                } else {
                    endWorld = getCanvasLocalPointRef.current((gesture as any).currentX, (gesture as any).currentY);
                }
            } else if (pendingConnectionRef.current && menuPosRef.current) {
                activeStart = pendingConnectionRef.current;
                endWorld = getCanvasLocalPointRef.current(menuPosRef.current.x, menuPosRef.current.y);
            }

            if (!activeStart || !endWorld) return;

            const viewport = useViewportStore.getState().viewports[groupId] || { x: 0, y: 0, scale: 1 };
            const currentTheme = themeRef.current;

            ctx.save();
            ctx.translate(viewport.x, viewport.y);
            ctx.scale(viewport.scale, viewport.scale);

            const start = getPinWorldPosRef.current(activeStart.id);
            if (start) {
                drawEdge(
                    ctx,
                    start.x, start.y,
                    endWorld.x, endWorld.y,
                    activeStart.ui?.color ?? getPinTypeColor(activeStart.type ?? "any", currentTheme),
                    2 / viewport.scale,
                    activeStart.direction === "input"
                );
            }

            ctx.restore();
        };

        renderRef.current = render;
        const unsubGesture = useGestureStore.subscribe(render);
        const unsubViewport = useViewportStore.subscribe(render);
        render();

        return () => { unsubGesture(); unsubViewport(); };
    }, [groupId]);

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
