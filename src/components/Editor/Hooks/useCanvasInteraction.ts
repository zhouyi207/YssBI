import React, { useState, useCallback, useEffect } from "react";
import { useNodeStore } from "../Store/useNodeStore";
import { useGestureStore } from "../Store/useGestureStore";
import { useViewportStore } from "../Store/useViewportStore";
import { BaseNode, Pin } from "../Types/nodes";
import { CanvasState, Gesture, EditorGroup } from "../Types/canvas";
import { isCompatiblePins, isSingleLinkPin } from "../Utils/pinUtils";
import { clamp } from "../../../types";

// Helper function moved from CanvasProvider
const updatePinLink = (node: BaseNode, pId: string, oId: string) => {
    const p = [...node.inputs, ...node.outputs].find((x) => x.id === pId);
    if (!p) return false;
    if (isSingleLinkPin(p)) {
        if (p.links.length === 1 && p.links[0] === oId) return false;
        p.links = [oId];
    } else {
        if (p.links.includes(oId)) return false;
        p.links = [...p.links, oId];
    }
    return true;
};

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

    const connectPins = useCallback((a: string, b: string) => {
        const tid = activeTabIdRef.current;
        if (!tid) return;
        const currentNodes = useNodeStore.getState().getNodes(tid);

        const findPin = (id: string) => {
            for (const n of currentNodes) { const p = [...n.inputs, ...n.outputs].find(x => x.id === id); if (p) return { pin: p, node: n }; }
            return null;
        };
        const resA = findPin(a); const resB = findPin(b);
        if (!resA || !resB || !isCompatiblePins(resA.pin, resB.pin)) return;
        saveHistory();
        setNodes((prev: BaseNode[]) => prev.map(n => {
            const newNode = n.clone(); let changed = false;
            const oldLinksToRemove = new Set<string>();
            if (isSingleLinkPin(resA.pin) && resA.pin.links.length > 0) resA.pin.links.forEach(l => oldLinksToRemove.add(l));
            if (isSingleLinkPin(resB.pin) && resB.pin.links.length > 0) resB.pin.links.forEach(l => oldLinksToRemove.add(l));
            [...newNode.inputs, ...newNode.outputs].forEach(p => { if (oldLinksToRemove.has(p.id)) { p.links = p.links.filter(l => l !== a && l !== b); changed = true; } });
            if (n.id === resA.node.id) if (updatePinLink(newNode, a, b)) changed = true;
            if (n.id === resB.node.id) if (updatePinLink(newNode, b, a)) changed = true;
            return changed ? newNode : n;
        }));
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

    const onPinPointerDown = useCallback((pinId: string, e: React.PointerEvent, groupId?: string) => {
        e.stopPropagation();

        // Alt + Click to Disconnect (Optimized)
        if (e.altKey && e.button === 0) {
            saveHistory();
            setNodes(prev => {
                // 找出哪些节点包含目标 Pin 或与其连接的 Pin
                const targetPinId = pinId;
                const nodeIdsToUpdate = new Set<string>();
                const linkedPinIds = new Set<string>();

                for (const node of prev) {
                    const allPins = [...node.inputs, ...node.outputs];
                    const p = allPins.find(x => x.id === targetPinId);
                    if (p) {
                        nodeIdsToUpdate.add(node.id);
                        p.links.forEach(l => linkedPinIds.add(l));
                    }
                }

                if (linkedPinIds.size > 0) {
                    for (const node of prev) {
                        if (nodeIdsToUpdate.has(node.id)) continue;
                        const allPins = [...node.inputs, ...node.outputs];
                        if (allPins.some(p => linkedPinIds.has(p.id))) {
                            nodeIdsToUpdate.add(node.id);
                        }
                    }
                }

                if (nodeIdsToUpdate.size === 0) return prev;

                return prev.map(n => {
                    if (!nodeIdsToUpdate.has(n.id)) return n;
                    const newNode = n.clone();
                    let changed = false;
                    [...newNode.inputs, ...newNode.outputs].forEach(p => {
                        if (p.id === targetPinId) {
                            if (p.links.length > 0) { p.links = []; changed = true; }
                        } else if (p.links.includes(targetPinId)) {
                            p.links = p.links.filter(l => l !== targetPinId);
                            changed = true;
                        }
                    });
                    return changed ? newNode : n;
                });
            });
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
                    // Fix: Use activeGroupIdRef to find the group and its selected nodes
                    const gid = g.groupId || activeGroupIdRef.current;
                    const group = groups.find(grp => grp.id === gid);
                    const sIds = group?.selectedNodeIds || [];

                    const tid = activeTabIdRef.current;
                    if (tid) {
                        sIds.forEach(id => {
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
