import { useRef, useCallback, useEffect, useLayoutEffect } from "react";
import { useViewportStore } from "../Store/useViewportStore";
import { useNodeStore } from "../Store/useNodeStore";
import { drawEdge } from "../Edges/Edge";
import { DEFAULT_VIEWPORT } from "./constants";

export const EdgesLayer = ({
    groupId,
    visibleNodeIds,
    pinNodeIdIndex,
    getPinWorldPos,
    getCanvasLocalPoint,
    gesture,
    pendingConnection, // Kept for dependency array compatibility, though logic moved? Code shows it was used in dependency array.
    contextMenu,
    activeTabId,
    theme
}: any) => {
    const edgeCanvasRef = useRef<HTMLCanvasElement>(null);
    const rafRef = useRef<number | null>(null);

    // 绘制连接线的核心逻辑 (GPU 加速)
    const drawAllEdges = useCallback(() => {
        const canvasEl = edgeCanvasRef.current;
        if (!canvasEl) return;
        const ctx = canvasEl.getContext("2d");
        if (!ctx) return;

        const canvas = useViewportStore.getState().viewports[groupId] || DEFAULT_VIEWPORT;

        // 清除画布
        ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);

        // 设置变换矩阵 (同步画布的平移和缩放)
        ctx.save();
        ctx.translate(canvas.x, canvas.y);
        ctx.scale(canvas.scale, canvas.scale);

        // 绘制已有连接
        const nodes = useNodeStore.getState().getNodes(activeTabId || "");

        nodes.forEach(node => {
            // const node = allNodes[id]; // Removed
            if (!node) return;

            // 暂时移除 Edges 层的视口裁剪判断，确保所有连接线都能绘制
            // 因为 getPinWorldPos 内部已经处理了未渲染节点的 null 返回

            node.outputs.forEach((pin: any) => {
                pin.links.forEach((targetId: string) => {
                    const start = getPinWorldPos(pin.id);
                    const end = getPinWorldPos(targetId);
                    if (!start || !end) return;

                    drawEdge(
                        ctx,
                        start.x, start.y,
                        end.x, end.y,
                        pin.ui?.color ?? (theme[`${pin.type}Color` as keyof typeof theme] as string) ?? theme.connectionLines,
                        2 / canvas.scale // 保持视觉粗细一致
                    );
                });
            });
        });


        // Removed interaction drawing from EdgesLayer as it's now handled by useGestureStore and ConnectionLine

        ctx.restore();
    }, [gesture, pendingConnection, contextMenu, getPinWorldPos, getCanvasLocalPoint, theme, groupId, activeTabId, visibleNodeIds, pinNodeIdIndex]);

    const requestDraw = useCallback(() => {
        if (rafRef.current) return;
        rafRef.current = requestAnimationFrame(() => {
            drawAllEdges();
            rafRef.current = null;
        });
    }, [drawAllEdges]);

    // 监听 ViewportStore 和 NodeStore 的变化，触发重绘
    useEffect(() => {
        const unsubViewport = useViewportStore.subscribe(() => {
            requestDraw();
        });
        const unsubNodes = useNodeStore.subscribe(() => {
            requestDraw();
        });
        return () => {
            unsubViewport();
            unsubNodes();
        };
    }, [requestDraw]);

    // 同步画布尺寸并触发重绘
    useLayoutEffect(() => {
        const canvasEl = edgeCanvasRef.current;
        if (!canvasEl) return;

        // 确保从正确的父元素获取尺寸
        const rect = canvasEl.parentElement?.getBoundingClientRect();
        if (!rect || rect.width === 0 || rect.height === 0) return;

        const dpr = window.devicePixelRatio || 1;

        // 设置实际像素大小 (防止模糊)
        canvasEl.width = rect.width * dpr;
        canvasEl.height = rect.height * dpr;
        // 设置 CSS 大小
        canvasEl.style.width = `${rect.width}px`;
        canvasEl.style.height = `${rect.height}px`;

        const ctx = canvasEl.getContext("2d");
        if (ctx) {
            ctx.setTransform(dpr, 0, 0, dpr, 0, 0); // 使用 setTransform 替代 scale 避免累加
        }

        drawAllEdges();
    }, [drawAllEdges]);

    return <canvas ref={edgeCanvasRef} className="absolute inset-0 pointer-events-none" />;
};
