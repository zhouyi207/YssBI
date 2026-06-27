import React, { useCallback, useEffect, useRef } from "react";
import { getGraphById } from "@/features/core/dataStore";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useGestureStore } from "@/features/core/gesture";
import { useViewportStore } from '@/features/core/viewport';
import { useEditorStore } from "@/features/core/editor";
import { executeCommand } from "@/features/core/history";
import { Node } from '@/shared/types/ui';
import { GraphPosition, Pin } from "@/shared/types/domain";
import { EditorGesture, EditorGroup } from "@/shared/types/ui";
import { logger } from '@/utils/appLogger';
import { ProjectService } from "@/services/project/projectService";
import { canConnectPins } from "@/shared/utils/pinCompatibility";

import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import {
    getCanvasWorldPoint,
    getGestureScreenMovement,
    hasSelectionChanged,
    selectNodeIdsInScreenRect,
} from "./canvasInteractionUtils";

interface UseCanvasInteractionProps {
    activeGroupIdRef: React.RefObject<string>;
    activeTabIdRef: React.RefObject<string | null>;
    canvasRef: React.RefObject<GraphPosition>;
    groups: EditorGroup[];
    setSelectedNodeIds: (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => void;
    setNodes: (updater: Node[] | ((prev: Node[]) => Node[])) => void;
    /** 为 false 时不注册全局 pointer 监听器，供 Sidebar 等非 Canvas 组件使用 */
    enabled?: boolean;
}

function updateSelectionPreview(groupId: string, previousIds: string[], nextIds: string[]) {
    const canvasEl = document.querySelector(`[data-editor-group-id="${groupId}"]`);
    if (!canvasEl) return;

    const nextSet = new Set(nextIds);
    for (const id of previousIds) {
        if (!nextSet.has(id)) {
            canvasEl.querySelector(`[data-node-id="${id}"]`)?.removeAttribute("data-selection-preview");
        }
    }

    const previousSet = new Set(previousIds);
    for (const id of nextIds) {
        if (!previousSet.has(id)) {
            canvasEl.querySelector(`[data-node-id="${id}"]`)?.setAttribute("data-selection-preview", "true");
        }
    }
}

export function useCanvasInteraction({
    activeGroupIdRef,
    activeTabIdRef,
    canvasRef,
    groups,
    setSelectedNodeIds,
    enabled = true,
}: UseCanvasInteractionProps) {

    // Use store instead of local state
    const contextMenu = useEditorStore((s) => s.contextMenu);
    const setContextMenu = useEditorStore((s) => s.setContextMenu);
    const pendingConnection = useEditorStore((s) => s.pendingConnection);
    const setPendingConnection = useEditorStore((s) => s.setPendingConnection);

    const selectionPreviewIdsRef = useRef<string[]>([]);

    const persistViewport = useCallback((graphId?: string | null) => {
        const tid = graphId ?? activeTabIdRef.current;
        if (!tid) return;
        const viewport = useViewportStore.getState().viewports[tid];
        if (viewport) {
            ProjectService.updateCanvas(tid, viewport).catch(() => {});
        }
    }, [activeTabIdRef]);

    const connectPins = useCallback(async (a: string, b: string) => {
        const tid = activeTabIdRef.current;
        if (!tid) return;

        const graph = getGraphById(tid);
        const pinA = graph?.pins.find((pin) => pin.id === a);
        const pinB = graph?.pins.find((pin) => pin.id === b);
        if (pinA && pinB && !canConnectPins(pinA as Pin, pinB as Pin)) {
            logger.graph.warn('Ignored type-mismatched pin connection attempt', 'CanvasInteraction');
            return;
        }

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
            selectionPreviewIdsRef.current = [];
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



    const onPinPointerDown = useCallback(async (pin: Pin, e: React.PointerEvent, groupId?: string) => {
        e.stopPropagation();

        if (e.altKey && e.button === 0) {
            const tid = activeTabIdRef.current;
            if (!tid) return;

            try {
                await executeCommand(tid, 'DisconnectPin', { pinId: pin.id });
            } catch (error) {
                logger.graph.error(`Failed to disconnect pin: ${error instanceof Error ? error.message : String(error)}`, 'CanvasInteraction');
            }
            return;
        }

        if (e.button !== 0) return;

        const tid = activeTabIdRef.current;
        if (!tid) return;

        // 计算初始世界坐标，避免第一帧在多 editor 中终点不一致
        const gid = groupId || activeGroupIdRef.current;
        const { x: worldX, y: worldY } = getCanvasWorldPoint(gid, tid, e.clientX, e.clientY);
        useGestureStore.getState().setGesture({ type: "connect", startPin: pin, startX: e.clientX, startY: e.clientY, currentX: e.clientX, currentY: e.clientY, worldX, worldY, groupId });
    }, [activeGroupIdRef, activeTabIdRef]);


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
                const layoutGroupId = g.groupId || activeGroupIdRef.current;
                const layoutNode = useLayoutStore.getState().nodes[layoutGroupId];
                const graphId = layoutNode?.data?.activeTabId ?? activeTabIdRef.current;
                if (graphId) {
                    useViewportStore.getState().setViewport(graphId, (prev: GraphPosition) => ({
                        ...prev,
                        x: prev.x + dx,
                        y: prev.y + dy
                    }));
                }
                nextGesture = { ...g, lastX: e.clientX, lastY: e.clientY, moved: true };
            } else if (g.type === "select") {
                nextGesture = { ...g, currentX: e.clientX, currentY: e.clientY };
                const x1 = Math.min(g.startX, e.clientX), y1 = Math.min(g.startY, e.clientY);
                const x2 = Math.max(g.startX, e.clientX), y2 = Math.max(g.startY, e.clientY);
                const gid = g.groupId || activeGroupIdRef.current;
                const layoutNode = useLayoutStore.getState().nodes[gid];
                const tabId = layoutNode?.data?.activeTabId ?? activeTabIdRef.current ?? null;
                const graphData = getGraphById(tabId || "");
                const newSelectedIds = selectNodeIdsInScreenRect(gid, graphData?.nodes ?? [], { x1, y1, x2, y2 });
                updateSelectionPreview(gid, selectionPreviewIdsRef.current, newSelectedIds);
                selectionPreviewIdsRef.current = newSelectedIds;
            }
            else if (g.type === "connect") {
                // 计算世界坐标，使多 editor 渲染同一连接线时终点一致
                const gid = g.groupId || activeGroupIdRef.current;
                const tid = activeTabIdRef.current;
                const { x: worldX, y: worldY } = getCanvasWorldPoint(gid, tid, e.clientX, e.clientY);
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
                    const layoutGroupId = g.groupId || activeGroupIdRef.current;
                    const layoutNode = useLayoutStore.getState().nodes[layoutGroupId];
                    const graphId = layoutNode?.data?.activeTabId ?? activeTabIdRef.current;
                    persistViewport(graphId);
                }
            }
            else if (g.type === "select") {
                const rect = g;
                const x1 = Math.min(rect.startX, e.clientX), y1 = Math.min(rect.startY, e.clientY);
                const x2 = Math.max(rect.startX, e.clientX), y2 = Math.max(rect.startY, e.clientY);
                const gid = g.groupId || activeGroupIdRef.current;
                const layoutNode = useLayoutStore.getState().nodes[gid];
                const tabId = layoutNode?.data?.activeTabId ?? activeTabIdRef.current ?? null;
                const graphData = getGraphById(tabId || "");
                const newSelectedIds = selectNodeIdsInScreenRect(gid, graphData?.nodes ?? [], { x1, y1, x2, y2 });
                updateSelectionPreview(gid, selectionPreviewIdsRef.current, []);
                selectionPreviewIdsRef.current = [];
                const current = useLayoutStore.getState().nodes[gid]?.data?.params?.selectedNodeIds ?? [];
                if (hasSelectionChanged(current, newSelectedIds)) {
                    setSelectedNodeIds(newSelectedIds, gid);
                }
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
            const hadMovement = getGestureScreenMovement(g, canvasRef.current?.scale ?? 1);
            useGestureStore.getState().clearGesture(hadMovement);
            updateSelectionPreview(g.groupId || activeGroupIdRef.current, selectionPreviewIdsRef.current, []);
            selectionPreviewIdsRef.current = [];
        };

        const cleanupPointerMove = addGlobalEventListener(window, "pointermove", onMove);
        const cleanupPointerUp = addGlobalEventListener(window, "pointerup", onUp);
        return () => {
            cleanupPointerMove();
            cleanupPointerUp();
            if (rAFId) cancelAnimationFrame(rAFId);
            updateSelectionPreview(activeGroupIdRef.current, selectionPreviewIdsRef.current, []);
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
        onPinPointerDown
    };
}
