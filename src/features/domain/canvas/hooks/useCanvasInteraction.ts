import React, { useCallback, useEffect } from "react";
import { getGraphById, useGraphDataStore } from "@/features/core/dataStore";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useGestureStore } from "@/features/core/gesture";
import { useViewportStore } from '@/features/core/viewport';
import { useEditorStore } from "@/features/core/editor";
import { Node } from '@/shared/types/ui';
import { Pin, Graph, GraphPosition } from "@/shared/types/domain";
import { EditorGesture, EditorGroup } from "@/shared/types/ui";

import { clamp } from "@/shared/utils";
import { ConnectionService } from "@/services";
import { deserializeGraph } from "@/shared/utils/editor";

interface UseCanvasInteractionProps {
    activeGroupIdRef: React.MutableRefObject<string>;
    activeTabIdRef: React.MutableRefObject<string | null>;
    canvasRef: React.MutableRefObject<GraphPosition>;
    groups: EditorGroup[];
    setSelectedNodeIds: (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => void;
    setNodes: (updater: Node[] | ((prev: Node[]) => Node[])) => void;
    setCanvas: (updater: GraphPosition | ((prev: GraphPosition) => GraphPosition), targetGroupId?: string) => void;
    saveHistory: () => void;
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
    saveHistory,
    enabled = true,
}: UseCanvasInteractionProps) {

    // Use store instead of local state
    const contextMenu = useEditorStore((s) => s.contextMenu);
    const setContextMenu = useEditorStore((s) => s.setContextMenu);
    const pendingConnection = useEditorStore((s) => s.pendingConnection);
    const setPendingConnection = useEditorStore((s) => s.setPendingConnection);

    const connectPins = useCallback(async (a: string, b: string) => {
        const tid = activeTabIdRef.current;
        if (!tid) return;

        try {
            console.log(`[useCanvasInteraction] Connecting pins via backend: ${a} -> ${b}`);
            await ConnectionService.connectPins(tid, a, b);
            saveHistory();
        } catch (error) {
            console.error('[useCanvasInteraction] Failed to connect pins:', error);
        }
    }, [activeTabIdRef, saveHistory]);

    const onCanvasPointerDown = useCallback((e: React.PointerEvent, groupId?: string) => {
        // Button 1 (Middle) or Button 2 (Right) or Alt+Left (Button 0) for panning
        if (e.button === 1 || e.button === 2 || (e.button === 0 && e.altKey)) {
            useGestureStore.getState().setGesture({ type: "pan", lastX: e.clientX, lastY: e.clientY, moved: false, groupId });
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
        useGestureStore.getState().setGesture({ type: "drag", nodeId, lastX: e.clientX, lastY: e.clientY, moved: false, groupId: gid });
    }, [activeGroupIdRef, groups, setSelectedNodeIds]);



    const onPinPointerDown = useCallback(async (pinId: string, e: React.PointerEvent, groupId?: string) => {
        e.stopPropagation();

        // Alt + Click to Disconnect (后端优先)
        if (e.altKey && e.button === 0) {
            const tid = activeTabIdRef.current;
            if (!tid) return;

            try {
                console.log(`[useCanvasInteraction] Disconnecting pin via backend: ${pinId}`);
                const updatedSerializedNodes = await ConnectionService.disconnectPin(tid, pinId);
                
                // 获取最新的连接列表
                const connections = await ConnectionService.getConnections(tid);

                const tempGraph: Graph = {
                    id: tid,
                    name: "temp",
                    type: "event",
                    nodes: updatedSerializedNodes as any[],
                    pins: [],
                    connections: { connections },  // Connection 类型包含 connections 数组
                    canvas: { x: 0, y: 0, scale: 1 }
                };

                const { nodes: newNodes } = deserializeGraph(tempGraph);

                setNodes(newNodes);
                saveHistory();
            } catch (error) {
                console.error('[useCanvasInteraction] Failed to disconnect pin:', error);
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

        useGestureStore.getState().setGesture({ type: "connect", startPin: pin, startX: e.clientX, startY: e.clientY, currentX: e.clientX, currentY: e.clientY, groupId });
    }, [activeTabIdRef, saveHistory, setNodes]);


    const onCanvasWheel = useCallback((e: React.WheelEvent, targetGroupId?: string) => {
        if (e.ctrlKey) {
            e.preventDefault(); const delta = -e.deltaY; const factor = Math.pow(1.1, delta / 100);
            setCanvas((prev: GraphPosition) => ({ ...prev, scale: clamp(prev.scale * factor, 0.1, 5) }), targetGroupId);
        } else { setCanvas((prev: GraphPosition) => ({ ...prev, x: prev.x - e.deltaX, y: prev.y - e.deltaY }), targetGroupId); }
    }, [setCanvas]);

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
            }
            else if (g.type === "connect") {
                nextGesture = { ...g, currentX: e.clientX, currentY: e.clientY };
                useGestureStore.getState().updateConnection(e.clientX, e.clientY);
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
                // 仅更新 gesture（含 dragDelta），不写 graphDataStore，减少卡顿
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
                }
            }
            else if (g.type === "select") {
                const rect = g;
                const x1 = Math.min(rect.startX, rect.currentX), y1 = Math.min(rect.startY, rect.currentY);
                const x2 = Math.max(rect.startX, rect.currentX), y2 = Math.max(rect.startY, rect.currentY);
                const gid = g.groupId || activeGroupIdRef.current;
                const newSelectedIds: string[] = [];

                const graphData = getGraphById(activeTabIdRef.current || "");
                const currentNodes = graphData ? deserializeGraph(graphData).nodes : [];
                currentNodes.forEach((n: any) => {
                    const el = document.getElementById(n.id); if (!el) return;
                    const r = el.getBoundingClientRect();
                    const overlap = !(r.left > x2 || r.right < x1 || r.top > y2 || r.bottom < y1);
                    if (overlap) newSelectedIds.push(n.id);
                });
                setSelectedNodeIds(newSelectedIds, gid);
            } else if (g.type === "connect") {
                const target = (e.target as HTMLElement).closest("[data-pin-id]");
                if (target) connectPins(g.startPin.id, target.getAttribute("data-pin-id")!);
                else { setPendingConnection(g.startPin); setContextMenu({ x: e.clientX, y: e.clientY, visible: true }); }
            } else if (g.type === "drag" && g.moved) {
                // 拖拽结束：将 dragDelta 写回 store
                const delta = g.dragDelta || { x: 0, y: 0 };
                if (Math.abs(delta.x) > 0.001 || Math.abs(delta.y) > 0.001) {
                    const gid = g.groupId || activeGroupIdRef.current;
                    const layoutNode = useLayoutStore.getState().nodes[gid];
                    const sIds = layoutNode?.data?.params?.selectedNodeIds || [];
                    const tid = activeTabIdRef.current;
                    if (tid && sIds.length > 0) {
                        const store = useGraphDataStore.getState();
                        const updates: Array<{ nodeId: string; x: number; y: number }> = [];
                        for (const id of sIds) {
                            const node = store.nodes[id];
                            if (node?.position) {
                                updates.push({
                                    nodeId: id,
                                    x: node.position.x + delta.x,
                                    y: node.position.y + delta.y,
                                });
                            }
                        }
                        if (updates.length > 0) {
                            store.batchUpdateNodePositions(updates);
                        }
                    }
                }
                saveHistory();
            }

            useGestureStore.getState().endConnection();
            useGestureStore.getState().setGesture(null);
        };

        window.addEventListener("pointermove", onMove); window.addEventListener("pointerup", onUp);
        return () => {
            window.removeEventListener("pointermove", onMove);
            window.removeEventListener("pointerup", onUp);
            if (rAFId) cancelAnimationFrame(rAFId);
        };
    }, [enabled, activeGroupIdRef, activeTabIdRef, canvasRef, connectPins, saveHistory, setSelectedNodeIds]);

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
