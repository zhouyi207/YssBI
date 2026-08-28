import { useRef, useEffect, useLayoutEffect } from "react";
import { subscribeToViewport, getViewport, useTheme, getPinTypeColor, type ViewportScope } from '@/features/application/viewCapabilities';
import { drawEdge } from "./Edge";


import { Pin } from "@/shared/types/domain";
import { resolvePinVisualSpec } from "@/shared/types/domain/pinVisual";
import { getConnectPreview, subscribeConnectPreview, type ConnectionFeedback } from '@/features/application/viewCapabilities';
import type { ThemeStatusPalette } from '@/shared/theme/themeTokens';

export function connectionFeedbackAttributes(feedback: ConnectionFeedback | null) {
    if (!feedback) return {};
    return feedback.kind === 'invalid'
        ? {
            'data-connection-feedback': feedback.kind,
            'data-connection-invalid-reason': feedback.reason,
        }
        : { 'data-connection-feedback': feedback.kind };
}

export function connectionFeedbackColor(
    feedback: ConnectionFeedback | null,
    fallback: string,
    palette: Pick<ThemeStatusPalette, 'success' | 'warning' | 'danger'>,
): string {
    if (!feedback) return fallback;
    if (feedback.kind === 'invalid') return palette.danger;
    return feedback.kind === 'replace' ? palette.warning : palette.success;
}

export const ConnectionLine = ({
    viewportScope,
    getPinWorldPos,
    getCanvasLocalPoint,
    pendingConnection,
    menuPos,
}: {
    viewportScope: ViewportScope | null;
    getPinWorldPos: (pinId: string) => { x: number; y: number } | null;
    getCanvasLocalPoint: (x: number, y: number) => { x: number; y: number };
    pendingConnection?: Pin | null;
    menuPos?: { x: number; y: number } | null;
}) => {
    const lineCanvasRef = useRef<HTMLCanvasElement>(null);
    const { tokens } = useTheme();

    const getPinWorldPosRef = useRef(getPinWorldPos);
    const getCanvasLocalPointRef = useRef(getCanvasLocalPoint);
    const themeTokensRef = useRef(tokens);
    const pendingConnectionRef = useRef(pendingConnection);
    const menuPosRef = useRef(menuPos);
    const renderRef = useRef<() => void>(() => {});

    useEffect(() => { getPinWorldPosRef.current = getPinWorldPos; }, [getPinWorldPos]);
    useEffect(() => { getCanvasLocalPointRef.current = getCanvasLocalPoint; }, [getCanvasLocalPoint]);
    useEffect(() => { themeTokensRef.current = tokens; }, [tokens]);
    useEffect(() => { pendingConnectionRef.current = pendingConnection; }, [pendingConnection]);
    useEffect(() => { menuPosRef.current = menuPos; }, [menuPos]);

    useEffect(() => {
        renderRef.current();
    }, [pendingConnection, menuPos]);

    useEffect(() => {
        const render = () => {
            const scope = viewportScope
              ? { graphPath: viewportScope.graphPath, groupId: viewportScope.groupId }
              : null;
            const connectPreview = scope ? getConnectPreview(scope) : null;
            const isPaneConnect = Boolean(connectPreview?.active && connectPreview.startPin);
            const gestureStartPin = isPaneConnect ? connectPreview?.startPin ?? null : null;
            const hasPendingConnection = pendingConnectionRef.current && menuPosRef.current;

            const canvasEl = lineCanvasRef.current;
            if (!canvasEl) return;
            const ctx = canvasEl.getContext("2d");
            if (!ctx) return;

            ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);

            if (!gestureStartPin && !hasPendingConnection) return;

            let activeStart = null;
            let endWorld: { x: number, y: number } | null = null;

            if (isPaneConnect && gestureStartPin) {
                activeStart = gestureStartPin;
                endWorld = { x: connectPreview!.worldX, y: connectPreview!.worldY };
            } else if (hasPendingConnection && menuPosRef.current) {
                activeStart = pendingConnectionRef.current;
                endWorld = getCanvasLocalPointRef.current(menuPosRef.current.x, menuPosRef.current.y);
            }

            if (!activeStart || !endWorld) return;

            const viewport = viewportScope ? getViewport(viewportScope) : { x: 0, y: 0, scale: 1 };
            const currentTheme = themeTokensRef.current;

            ctx.save();
            ctx.translate(viewport.x, viewport.y);
            ctx.scale(viewport.scale, viewport.scale);

            const start = getPinWorldPosRef.current(activeStart.id);
            if (start) {
                const colorKey = resolvePinVisualSpec(activeStart).colorKey;
                drawEdge(
                    ctx,
                    start.x, start.y,
                    endWorld.x, endWorld.y,
                    connectionFeedbackColor(
                        connectPreview?.feedback ?? null,
                        getPinTypeColor(colorKey, currentTheme),
                        currentTheme.status,
                    ),
                    2,
                    activeStart.direction === "input"
                );
            }

            ctx.restore();
        };

        renderRef.current = render;
        const unsubPreview = subscribeConnectPreview(render);
        const unsubViewport = viewportScope ? subscribeToViewport(viewportScope, render) : () => {};
        render();

        return () => {
            unsubPreview();
            unsubViewport();
        };
    }, [viewportScope?.groupId, viewportScope?.graphPath]);

    useLayoutEffect(() => {
        const canvasEl = lineCanvasRef.current;
        if (!canvasEl) return;
        const parent = canvasEl.parentElement;
        if (!parent) return;

        const syncCanvasSize = () => {
            const rect = parent.getBoundingClientRect();
            if (rect.width <= 0 || rect.height <= 0) return;

            const dpr = window.devicePixelRatio || 1;
            const backingWidth = Math.round(rect.width * dpr);
            const backingHeight = Math.round(rect.height * dpr);
            if (canvasEl.width !== backingWidth) canvasEl.width = backingWidth;
            if (canvasEl.height !== backingHeight) canvasEl.height = backingHeight;
            canvasEl.style.width = `${rect.width}px`;
            canvasEl.style.height = `${rect.height}px`;
            const ctx = canvasEl.getContext('2d');
            if (ctx) ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
            renderRef.current();
        };

        syncCanvasSize();
        const observer = new ResizeObserver(syncCanvasSize);
        observer.observe(parent);
        return () => observer.disconnect();
    }, []);

    const preview = viewportScope
      ? getConnectPreview({ graphPath: viewportScope.graphPath, groupId: viewportScope.groupId })
      : null;
    return (
        <canvas
            ref={lineCanvasRef}
            className="absolute inset-0 pointer-events-none z-50"
            {...connectionFeedbackAttributes(preview?.feedback ?? null)}
        />
    );
};
