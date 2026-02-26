import React, { useCallback, useEffect, useRef } from "react";
import { getGraphById } from "@/features/core/dataStore";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useGestureStore } from "@/features/core/gesture";
import { useViewportStore } from '@/features/core/viewport';
import { useEditorStore } from "@/features/core/editor";
import { executeCommand } from "@/features/core/history";
import { Node } from '@/shared/types/ui';
import { Pin, GraphPosition } from "@/shared/types/domain";
import { EditorGesture, EditorGroup } from "@/shared/types/ui";
import { logger } from '@/utils/appLogger';
import { ProjectService } from "@/services/project/projectService";

import { clamp } from "@/shared/utils";
import { deserializeGraph } from "@/features/core/dataStore";
import { CONTEXT_MENU_MOVE_THRESHOLD_PX } from "@/app/appConfig/default";

interface UseCanvasInteractionProps {
    activeGroupIdRef: React.RefObject<string>;
    activeTabIdRef: React.RefObject<string | null>;
    canvasRef: React.RefObject<GraphPosition>;
    groups: EditorGroup[];
    setSelectedNodeIds: (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => void;
    setNodes: (updater: Node[] | ((prev: Node[]) => Node[])) => void;
    setCanvas: (updater: GraphPosition | ((prev: GraphPosition) => GraphPosition), targetGroupId?: string) => void;
    /** 为 false 时不注册全局 pointer 监听器，供 Sidebar 等非 Canvas 组件使用 */
    enabled?: boolean;
}

export function useCanvasInteraction({
    activeGroupIdRef,
    activeTabIdRef,
    canvasRef,
    groups,
    setSelectedNodeIds,
    setNodes,
    setCanvas,
    enabled = true,
}: UseCanvasInteractionProps) {

    // Use store instead of local state
    const contextMenu = useEditorStore((s) => s.contextMenu);
    const setContextMenu = useEditorStore((s) => s.setContextMenu);
    const pendingConnection = useEditorStore((s) => s.pendingConnection);
    const setPendingConnection = useEditorStore((s) => s.setPendingConnection);

    const wheelSaveTimerRef = useRef<number | null>(null);

    const persistViewport = useCallback((groupId?: string) => {
        const gid = groupId || activeGroupIdRef.current;
        const lNode = useLayoutStore.getState().nodes[gid];
        const tid = lNode?.data?.activeTabId ?? activeTabIdRef.current;
        if (!tid) return;
        const viewport = useViewportStore.getState().viewports[gid];
        if (viewport) {
            ProjectService.updateCanvas(tid, viewport).catch(() => {});
        }
    }, [activeGroupIdRef, activeTabIdRef]);

    const connectPins = useCallback(async (a: string, b: string) => {
        const tid = activeTabIdRef.current;
        if (!tid) return;

        try {
            await executeCommand(tid, 'ConnectPins', { pinA: a, pinB: b });
        } catch (error) {
            logger.graph.error(`Failed to connect pins: ${error instanceof Error ? error.message : String(error)}`, 'CanvasInteraction');
        }
    }, [activeTabIdRef]);

    const onCanvasPointerDown = useCallback((e: React.PointerEvent, groupId?: string) => {
        // Button 1 (Middle) or Button 2 (Right) or Alt+Left (Button 0) for panning
        if (e.button === 1 || e.button === 2 || (e.button === 0 && e.altKey)) {
            useGestureStore.getState().setGesture({
                type: "pan",
                startX: e.clientX,
                startY: e.clientY,
                lastX: e.clientX,
                lastY: e.clientY,
                moved: false,
                groupId,
            });
            return;
        }
        if (e.button === 0) {
            if (!e.shiftKey) { setSelectedNodeIds([], groupId); }
            useGestureStore.getState().setGesture({ type: "select", startX: e.clientX, startY: e.clientY, currentX: e.clientX, currentY: e.clientY, groupId });
        }
    }, [setSelectedNodeIds]);

    const onNodePointerDown = useCallback((nodeId: string, e: React.PointerEvent, groupId?: string) => {
        e.stopPropagation(); if (e.button !== 0) return;
        const gid = groupId || activeGroupIdRef.current;
        const currentSelected = groups.find(g => g.id === gid)?.selectedNodeIds || [];

        if (e.shiftKey) {
            if (currentSelected.includes(nodeId)) {
                setSelectedNodeIds(prev => prev.filter(id => id !== nodeId), gid);
            } else {
                setSelectedNodeIds(prev => [...prev, nodeId], gid);
            }
        } else {
            if (!currentSelected.includes(nodeId)) {
                setSelectedNodeIds([nodeId], gid);
            }
        }
        // 记录被拖拽的节点 ID 列表，用于多 editor 同步
        const layoutNode = useLayoutStore.getState().nodes[gid];
        const dragNodeIds = layoutNode?.data?.params?.selectedNodeIds || [];
        // 确保当前点击的节点也在列表中
        const finalDragNodeIds = dragNodeIds.includes(nodeId) ? dragNodeIds : [nodeId, ...dragNodeIds];
        useGestureStore.getState().setGesture({ type: "drag", nodeId, lastX: e.clientX, lastY: e.clientY, moved: false, groupId: gid, dragNodeIds: finalDragNodeIds, dragDelta: { x: 0, y: 0 } });
    }, [activeGroupIdRef, groups, setSelectedNodeIds]);



    const onPinPointerDown = useCallback(async (pinId: string, e: React.PointerEvent, groupId?: string) => {
        e.stopPropagation();

        if (e.altKey && e.button === 0) {
            const tid = activeTabIdRef.current;
            if (!tid) return;

            try {
                await executeCommand(tid, 'DisconnectPin', { pinId });
            } catch (error) {
                logger.graph.error(`Failed to disconnect pin: ${error instanceof Error ? error.message : String(error)}`, 'CanvasInteraction');
            }
            return;
        }

        if (e.button !== 0) return;

        // Find pin in current store nodes
        const tid = activeTabIdRef.current;
        if (!tid) return;
        const graphData = getGraphById(tid);
        const currentNodes = graphData ? deserializeGraph(graphData).nodes : [];
        let pin: Pin | undefined;
        for (const n of currentNodes) {
            pin = [...n.inputs, ...n.outputs].find(p => p.id === pinId);
            if (pin) break;
        }
        if (!pin) return;

        // 计算初始世界坐标，避免第一帧在多 editor 中终点不一致
        const gid = groupId || activeGroupIdRef.current;
        const canvasEl = document.querySelector(`[data-editor-group-id="${gid}"]`);
        let worldX = e.clientX;
        let worldY = e.clientY;
        if (canvasEl) {
            const rect = canvasEl.getBoundingClientRect();
            const vp = useViewportStore.getState().viewports[gid] || { x: 0, y: 0, scale: 1 };
            worldX = (e.clientX - rect.left - vp.x) / vp.scale;
            worldY = (e.clientY - rect.top - vp.y) / vp.scale;
        }
        useGestureStore.getState().setGesture({ type: "connect", startPin: pin, startX: e.clientX, startY: e.clientY, currentX: e.clientX, currentY: e.clientY, worldX, worldY, groupId });
    }, [activeTabIdRef, setNodes]);


    const onCanvasWheel = useCallback((e: React.WheelEvent, targetGroupId?: string) => {
        if (e.ctrlKey) {
            e.preventDefault(); const delta = -e.deltaY; const factor = Math.pow(1.1, delta / 100);
            setCanvas((prev: GraphPosition) => ({ ...prev, scale: clamp(prev.scale * factor, 0.1, 5) }), targetGroupId);
        } else { setCanvas((prev: GraphPosition) => ({ ...prev, x: prev.x - e.deltaX, y: prev.y - e.deltaY }), targetGroupId); }

        if (wheelSaveTimerRef.current !== null) clearTimeout(wheelSaveTimerRef.current);
        wheelSaveTimerRef.current = window.setTimeout(() => {
            wheelSaveTimerRef.current = null;
            persistViewport(targetGroupId);
        }, 300);
    }, [setCanvas, persistViewport]);

    // Global Pointer Events (Move/Up) - 仅当 enabled 时注册，避免 Sidebar 等组件重复监听
    useEffect(() => {
        if (!enabled) return;
        let rAFId: number | null = null;
        let latestEvent: PointerEvent | null = null;

        const processMove = () => {
            if (!latestEvent) return;
            const e = latestEvent;
            latestEvent = null;
            rAFId = null;

            const g = useGestureStore.getState().gesture; if (!g) return;

            let nextGesture: EditorGesture = null;

            if (g.type === "pan") {
                const dx = e.clientX - g.lastX, dy = e.clientY - g.lastY;
                useViewportStore.getState().setViewport(g.groupId || activeGroupIdRef.current, (prev: GraphPosition) => ({
                    ...prev,
                    x: prev.x + dx,
                    y: prev.y + dy
                }));
                nextGesture = { ...g, lastX: e.clientX, lastY: e.clientY, moved: true };
            } else if (g.type === "select") {
                nextGesture = { ...g, currentX: e.clientX, currentY: e.clientY };
                // 实时选择：拖拽过程中立即更新框选中的节点（优化：避免重复更新、避免重反序列化）
                const x1 = Math.min(g.startX, e.clientX), y1 = Math.min(g.startY, e.clientY);
                const x2 = Math.max(g.startX, e.clientX), y2 = Math.max(g.startY, e.clientY);
                const gid = g.groupId || activeGroupIdRef.current;
                // 使用 gesture 所在 editor group 的 activeTabId，而非全局 activeTabIdRef
                const layoutNode = useLayoutStore.getState().nodes[gid];
                const tabId = layoutNode?.data?.activeTabId ?? activeTabIdRef.current ?? null;
                const graphData = getGraphById(tabId || "");
                const currentNodes = graphData?.nodes ?? [];
                const newSelectedIds: string[] = [];
                // 在 gesture 所在 editor 的 canvas 内查找节点，避免多 editor 同 graph 时 document.getElementById 返回错误 editor 的节点
                const canvasEl = document.querySelector(`[data-editor-group-id="${gid}"]`);
                for (const n of currentNodes) {
                    const nodeId = (n as { id?: string }).id;
                    if (!nodeId) continue;
                    const el = canvasEl
                        ? (canvasEl as Element).querySelector(`[data-node-id="${nodeId}"]`)
                        : document.getElementById(nodeId);
                    if (!el) continue;
                    const r = el.getBoundingClientRect();
                    if (!(r.left > x2 || r.right < x1 || r.top > y2 || r.bottom < y1)) {
                        newSelectedIds.push(nodeId);
                    }
                }
                // 仅当选择结果变化时才更新，减少不必要的 re-render
                const current = layoutNode?.data?.params?.selectedNodeIds ?? [];
                const newSet = new Set(newSelectedIds);
                const curSet = new Set(current);
                if (newSet.size !== curSet.size || [...newSet].some((id) => !curSet.has(id))) {
                    setSelectedNodeIds(newSelectedIds, gid);
                }
            }
            else if (g.type === "connect") {
                // 计算世界坐标，使多 editor 渲染同一连接线时终点一致
                const gid = g.groupId || activeGroupIdRef.current;
                const canvasEl = document.querySelector(`[data-editor-group-id="${gid}"]`);
                let worldX = e.clientX;
                let worldY = e.clientY;
                if (canvasEl) {
                    const rect = canvasEl.getBoundingClientRect();
                    const vp = useViewportStore.getState().viewports[gid] || { x: 0, y: 0, scale: 1 };
                    worldX = (e.clientX - rect.left - vp.x) / vp.scale;
                    worldY = (e.clientY - rect.top - vp.y) / vp.scale;
                }
                nextGesture = { ...g, currentX: e.clientX, currentY: e.clientY, worldX, worldY };
                useGestureStore.getState().setGesture(nextGesture);
                nextGesture = null; // already set above
            }
            else if (g.type === "drag") {
                const canvas = canvasRef.current || { scale: 1 };
                const scale = canvas.scale || 1;
                const dx = (e.clientX - g.lastX) / scale;
                const dy = (e.clientY - g.lastY) / scale;

                let moved = g.moved;
                let lastX = g.lastX;
                let lastY = g.lastY;
                const prevDelta = g.dragDelta || { x: 0, y: 0 };
                let dragDelta = prevDelta;

                if (Math.abs(dx) > 0.01 || Math.abs(dy) > 0.01) {
                    moved = true;
                    dragDelta = { x: prevDelta.x + dx, y: prevDelta.y + dy };
                    lastX = e.clientX;
                    lastY = e.clientY;
                }
                nextGesture = { ...g, moved, lastX, lastY, dragDelta };
            }

            if (nextGesture) {
                useGestureStore.getState().setGesture(nextGesture);
            }
        };

        const onMove = (e: PointerEvent) => {
            latestEvent = e;
            if (rAFId === null) {
                rAFId = requestAnimationFrame(processMove);
            }
        };

        const onUp = (e: PointerEvent) => {
            if (rAFId) {
                cancelAnimationFrame(rAFId);
                rAFId = null;
            }
            const g = useGestureStore.getState().gesture; if (!g) return;
            if (g.type === "pan") {
                if (!g.moved && e.button === 2) {
                    setContextMenu({ x: e.clientX, y: e.clientY, visible: true });
                } else if (g.moved) {
                    persistViewport(g.groupId);
                }
            }
            else if (g.type === "select") {
                const rect = g;
                const x1 = Math.min(rect.startX, rect.currentX), y1 = Math.min(rect.startY, rect.currentY);
                const x2 = Math.max(rect.startX, rect.currentX), y2 = Math.max(rect.startY, rect.currentY);
                const gid = g.groupId || activeGroupIdRef.current;
                // 使用 gesture 所在 editor group 的 activeTabId，而非全局 activeTabIdRef
                const layoutNode = useLayoutStore.getState().nodes[gid];
                const tabId = layoutNode?.data?.activeTabId ?? activeTabIdRef.current ?? null;
                const graphData = getGraphById(tabId || "");
                const currentNodes = graphData?.nodes ?? [];
                const newSelectedIds: string[] = [];
                // 在 gesture 所在 editor 的 canvas 内查找节点，避免多 editor 同 graph 时 document.getElementById 返回错误 editor 的节点
                const canvasEl = document.querySelector(`[data-editor-group-id="${gid}"]`);
                for (const n of currentNodes) {
                    const nodeId = (n as { id?: string }).id;
                    if (!nodeId) continue;
                    const el = canvasEl
                        ? (canvasEl as Element).querySelector(`[data-node-id="${nodeId}"]`)
                        : document.getElementById(nodeId);
                    if (!el) continue;
                    const r = el.getBoundingClientRect();
                    if (!(r.left > x2 || r.right < x1 || r.top > y2 || r.bottom < y1)) {
                        newSelectedIds.push(nodeId);
                    }
                }
                setSelectedNodeIds(newSelectedIds, gid);
            } else if (g.type === "connect") {
                const target = (e.target as HTMLElement).closest("[data-pin-id]");
                if (target) connectPins(g.startPin.id, target.getAttribute("data-pin-id")!);
                else { setPendingConnection(g.startPin); setContextMenu({ x: e.clientX, y: e.clientY, visible: true }); }
            } else if (g.type === "drag" && g.moved) {
                const delta = g.dragDelta || { x: 0, y: 0 };
                if (Math.abs(delta.x) > 0.001 || Math.abs(delta.y) > 0.001) {
                    const dragIds = g.dragNodeIds || [];
                    const gid = g.groupId || activeGroupIdRef.current;
                    const lNode = useLayoutStore.getState().nodes[gid];
                    const tid = lNode?.data?.activeTabId ?? activeTabIdRef.current;
                    if (tid && dragIds.length > 0) {
                        executeCommand(
                            tid,
                            'MoveNodes',
                            { nodeIds: dragIds, delta },
                            { mergeKey: `move-${[...dragIds].sort().join(',')}` },
                        ).catch((e) =>
                            logger.graph.warn(`MoveNodes command failed: ${e instanceof Error ? e.message : String(e)}`, 'CanvasInteraction')
                        );
                    }
                }
            }

            useGestureStore.getState().endConnection();
            const threshold = CONTEXT_MENU_MOVE_THRESHOLD_PX;
            let hadMovement = false;
            if (g.type === "pan") {
                const dx = g.lastX - g.startX;
                const dy = g.lastY - g.startY;
                hadMovement = Math.sqrt(dx * dx + dy * dy) > threshold;
            } else if (g.type === "select") {
                const dx = Math.abs(g.currentX - g.startX);
                const dy = Math.abs(g.currentY - g.startY);
                hadMovement = dx > threshold || dy > threshold;
            } else if (g.type === "drag" && g.dragDelta) {
                const scale = canvasRef.current?.scale ?? 1;
                const screenDx = Math.abs(g.dragDelta.x * scale);
                const screenDy = Math.abs(g.dragDelta.y * scale);
                hadMovement = screenDx > threshold || screenDy > threshold;
            }
            useGestureStore.getState().clearGesture(hadMovement);
        };

        window.addEventListener("pointermove", onMove); window.addEventListener("pointerup", onUp);
        return () => {
            window.removeEventListener("pointermove", onMove);
            window.removeEventListener("pointerup", onUp);
            if (rAFId) cancelAnimationFrame(rAFId);
            if (wheelSaveTimerRef.current !== null) clearTimeout(wheelSaveTimerRef.current);
        };
    }, [enabled, activeGroupIdRef, activeTabIdRef, canvasRef, connectPins, setSelectedNodeIds, persistViewport]);

    return {
        contextMenu,
        setContextMenu,
        pendingConnection,
        setPendingConnection,
        connectPins,
        onCanvasPointerDown,
        onNodePointerDown,
        onPinPointerDown,
        onCanvasWheel
    };
}
