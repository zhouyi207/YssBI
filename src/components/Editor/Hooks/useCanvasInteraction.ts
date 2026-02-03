import React, { useState, useCallback, useEffect } from "react";
import { useNodeStore } from "../Store/useNodeStore";
import { useGestureStore } from "../Store/useGestureStore";
import { useViewportStore } from "../Store/useViewportStore";
import { BaseNode, Pin } from "../Types/nodes";
import { CanvasState, Gesture, EditorGroup, SubGraphData } from "../Types/canvas";
import { clamp } from "../../../types";
import { ProjectService } from "../../../services/projectService";
import { deserializeSubGraph } from "../Utils/io";

interface UseCanvasInteractionProps {
    activeGroupIdRef: React.MutableRefObject<string>;
    activeTabIdRef: React.MutableRefObject<string | null>;
    canvasRef: React.MutableRefObject<CanvasState>;
    groups: EditorGroup[];
    setSelectedNodeIds: (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => void;
    setNodes: (updater: BaseNode[] | ((prev: BaseNode[]) => BaseNode[])) => void;
    setCanvas: (updater: CanvasState | ((prev: CanvasState) => CanvasState), targetGroupId?: string) => void;
    saveHistory: () => void;
}

export function useCanvasInteraction({
    activeGroupIdRef,
    activeTabIdRef,
    canvasRef,
    groups,
    setSelectedNodeIds,
    setNodes,
    setCanvas,
    saveHistory
}: UseCanvasInteractionProps) {

    const [contextMenu, setContextMenu] = useState<{ x: number, y: number, visible: boolean } | null>(null);
    const [pendingConnection, setPendingConnection] = useState<Pin | null>(null);

    const connectPins = useCallback(async (a: string, b: string) => {
        const tid = activeTabIdRef.current;
        if (!tid) return;

        try {
            console.log(`[useCanvasInteraction] Connecting pins via backend: ${a} -> ${b}`);
            
            // 调用后端 API 进行连接
            const updatedSerializedNodes = await ProjectService.connectPins(tid, a, b);
            
            // 获取最新的连接列表
            const connections = await ProjectService.getConnections(tid);
            
            console.log(`[useCanvasInteraction] Connection created, got ${connections.length} connections`);

            // 将返回的 SerializedNode[] 转换为 BaseNode[]
            // 构造临时的 SubGraphData 来复用 deserializeSubGraph 的逻辑
            const tempSubGraph: SubGraphData = {
                id: tid,
                name: "temp",
                type: "event",
                nodes: updatedSerializedNodes as any[],
                connections: connections,  // 包含连接数组
                canvas: { x: 0, y: 0, scale: 1 },
                variables: {},
                inputs: [],
                outputs: []
            };

            const { nodes: newNodes } = deserializeSubGraph(tempSubGraph);

            setNodes(newNodes);
            saveHistory();

        } catch (error) {
            console.error('[useCanvasInteraction] Failed to connect pins:', error);
            // 这里可以添加 toast 提示用户连接失败（例如类型不兼容）
        }
    }, [activeTabIdRef, saveHistory, setNodes]);

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
                const updatedSerializedNodes = await ProjectService.disconnectPin(tid, pinId);
                
                // 获取最新的连接列表
                const connections = await ProjectService.getConnections(tid);

                const tempSubGraph: SubGraphData = {
                    id: tid,
                    name: "temp",
                    type: "event",
                    nodes: updatedSerializedNodes as any[],
                    connections: connections,  // 包含连接数组
                    canvas: { x: 0, y: 0, scale: 1 },
                    variables: {},
                    inputs: [],
                    outputs: []
                };

                const { nodes: newNodes } = deserializeSubGraph(tempSubGraph);

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
        const currentNodes = useNodeStore.getState().getNodes(tid);
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
            setCanvas(prev => ({ ...prev, scale: clamp(prev.scale * factor, 0.1, 5) }), targetGroupId);
        } else { setCanvas(prev => ({ ...prev, x: prev.x - e.deltaX, y: prev.y - e.deltaY }), targetGroupId); }
    }, [setCanvas]);

    // Global Pointer Events (Move/Up)
    useEffect(() => {
        let rAFId: number | null = null;
        let latestEvent: PointerEvent | null = null;

        const processMove = () => {
            if (!latestEvent) return;
            const e = latestEvent;
            latestEvent = null;
            rAFId = null;

            const g = useGestureStore.getState().gesture; if (!g) return;

            let nextGesture: Gesture = null;

            if (g.type === "pan") {
                const dx = e.clientX - g.lastX, dy = e.clientY - g.lastY;
                useViewportStore.getState().setViewport(g.groupId || activeGroupIdRef.current, prev => ({
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

                if (Math.abs(dx) > 0.01 || Math.abs(dy) > 0.01) {
                    moved = true;
                    const gid = g.groupId || activeGroupIdRef.current;
                    const group = groups.find(grp => grp.id === gid);
                    const sIds = group?.selectedNodeIds || [];

                    const tid = activeTabIdRef.current;
                    if (tid) {
                        // 🆕 直接操作 DOM 以获得即时反馈
                        sIds.forEach(id => {
                            const nodeEl = document.querySelector(`[data-node-id="${id}"]`) as HTMLElement;
                            if (nodeEl) {
                                // 获取当前 transform
                                const currentTransform = nodeEl.style.transform;
                                const match = currentTransform.match(/translate3d\(([-\d.]+)px,\s*([-\d.]+)px,\s*([-\d.]+)px\)/);
                                if (match) {
                                    const currentX = parseFloat(match[1]);
                                    const currentY = parseFloat(match[2]);
                                    const newX = currentX + dx;
                                    const newY = currentY + dy;
                                    // 立即更新 DOM
                                    nodeEl.style.transform = `translate3d(${newX}px, ${newY}px, 0)`;
                                }
                            }
                            // 同时更新状态（用于持久化）
                            useNodeStore.getState().updateNodePosition(tid, id, dx, dy);
                        });
                    }
                    lastX = e.clientX;
                    lastY = e.clientY;
                }
                nextGesture = { ...g, moved, lastX, lastY };
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

                const currentNodes = useNodeStore.getState().getNodes(activeTabIdRef.current || "");
                currentNodes.forEach(n => {
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
                saveHistory();
                // 拖动结束后同步节点位置到后端
                const tid = activeTabIdRef.current;
                if (tid) {
                    console.log(`[useCanvasInteraction] Drag ended, syncing nodes to backend...`);
                    import('../Store/useNodeStore').then(({ useNodeStore }) => {
                        import('../Store/useProjectStore').then(({ useProjectStore }) => {
                            useProjectStore.getState().syncWithTabs(useNodeStore.getState().tabs);
                        });
                    });
                }
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
    }, [activeGroupIdRef, activeTabIdRef, groups, canvasRef, connectPins, saveHistory, setSelectedNodeIds]);

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
