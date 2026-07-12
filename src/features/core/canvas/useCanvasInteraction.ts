import React, { useCallback, useEffect, useMemo, useRef } from "react";
import { getGraphByPath } from "@/features/core/dataStore";
import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useGestureStore } from "@/features/core/gesture";
import { persistGraphViewport } from '@/features/core/viewport';
import { useEditorStore } from "@/features/core/editor";
import { executeCommand } from "@/features/core/history";
import { Pin } from "@/shared/types/domain";
import type { EditorViewport } from "@/features/core/viewport";
import { logger } from '@/utils/appLogger';
import { canConnectPins } from "@/shared/utils/pinCompatibility";

import { getCanvasWorldPoint, resolveTabId } from "./canvasInteractionUtils";
import { attachCanvasPointerLoop } from "./canvasPointerLoop";
import {
    startSelectionSession,
    abortSelectionSession,
} from "./selectionSession";

interface UseCanvasInteractionProps {
    activeGroupIdRef: React.RefObject<string>;
    activeTabIdRef: React.RefObject<string | null>;
    viewportRef: React.RefObject<EditorViewport>;
    setSelectedNodeIds: (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => void;
    /** 为 false 时不注册全局 pointer 监听器，供 Sidebar 等非 Canvas 组件使用 */
    enabled?: boolean;
}

export function useCanvasInteraction({
    activeGroupIdRef,
    activeTabIdRef,
    viewportRef,
    setSelectedNodeIds,
    enabled = true,
}: UseCanvasInteractionProps) {

    const contextMenu = useEditorStore((s) => s.contextMenu);
    const setContextMenu = useEditorStore((s) => s.setContextMenu);
    const pendingConnection = useEditorStore((s) => s.pendingConnection);
    const setPendingConnection = useEditorStore((s) => s.setPendingConnection);

    const setSelectedNodeIdsRef = useRef(setSelectedNodeIds);
    setSelectedNodeIdsRef.current = setSelectedNodeIds;

    const persistViewport = useCallback((graphPath?: string | null) => {
        persistGraphViewport(graphPath ?? activeTabIdRef.current);
    }, [activeTabIdRef]);

    const connectPins = useCallback(async (groupId: string, a: string, b: string) => {
        const tid = resolveTabId(groupId, activeTabIdRef);
        if (!tid) return;

        const graph = getGraphByPath(tid);
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
        if (e.button === 0 && groupId) {
            abortSelectionSession(groupId);
            startSelectionSession({
                groupId,
                startX: e.clientX,
                startY: e.clientY,
                preserveSelection: e.shiftKey,
            });
        }
    }, [activeTabIdRef]);

    const onNodePointerDown = useCallback((nodeId: string, e: React.PointerEvent, groupId?: string) => {
        e.stopPropagation();
        const gid = groupId || activeGroupIdRef.current;
        abortSelectionSession(gid);
        if (e.button !== 0) return;

        const layoutNode = useLayoutStore.getState().nodes[gid];
        const currentSelected = layoutNode?.data?.params?.selectedNodeIds || [];

        let dragNodeIds = currentSelected;
        if (e.shiftKey) {
            if (currentSelected.includes(nodeId)) {
                dragNodeIds = currentSelected.filter((id: string) => id !== nodeId);
                setSelectedNodeIdsRef.current(dragNodeIds, gid);
            } else {
                dragNodeIds = [...currentSelected, nodeId];
                setSelectedNodeIdsRef.current(dragNodeIds, gid);
            }
        } else if (!currentSelected.includes(nodeId)) {
            dragNodeIds = [nodeId];
            setSelectedNodeIdsRef.current([nodeId], gid);
        }

        const finalDragNodeIds = dragNodeIds.includes(nodeId) ? dragNodeIds : [nodeId, ...dragNodeIds];
        useGestureStore.getState().setGesture({ type: "drag", nodeId, lastX: e.clientX, lastY: e.clientY, moved: false, groupId: gid, dragNodeIds: finalDragNodeIds, dragDelta: { x: 0, y: 0 } });
    }, [activeGroupIdRef]);

    const onPinPointerDown = useCallback(async (pin: Pin, e: React.PointerEvent, groupId?: string) => {
        e.stopPropagation();
        abortSelectionSession(groupId || activeGroupIdRef.current);

        if (e.altKey && e.button === 0) {
            const gid = groupId || activeGroupIdRef.current;
            const tid = resolveTabId(gid, activeTabIdRef);
            if (!tid) return;

            try {
                await executeCommand(tid, 'DisconnectPin', { pinId: pin.id });
            } catch (error) {
                logger.graph.error(`Failed to disconnect pin: ${error instanceof Error ? error.message : String(error)}`, 'CanvasInteraction');
            }
            return;
        }

        if (e.button !== 0) return;

        const gid = groupId || activeGroupIdRef.current;
        const tid = resolveTabId(gid, activeTabIdRef);
        if (!tid) return;

        const { x: worldX, y: worldY } = getCanvasWorldPoint(gid, tid, e.clientX, e.clientY);
        useGestureStore.getState().setGesture({ type: "connect", startPin: pin, startX: e.clientX, startY: e.clientY, currentX: e.clientX, currentY: e.clientY, worldX, worldY, groupId });
    }, [activeGroupIdRef, activeTabIdRef]);

    useEffect(() => {
        if (!enabled) return;
        return attachCanvasPointerLoop({
            activeGroupIdRef,
            activeTabIdRef,
            viewportRef,
            setSelectedNodeIds: (updater, targetGroupId) => setSelectedNodeIdsRef.current(updater, targetGroupId),
            connectPins,
            persistViewport,
            setContextMenu,
            setPendingConnection,
        });
    }, [enabled, activeGroupIdRef, activeTabIdRef, viewportRef, connectPins, persistViewport, setContextMenu, setPendingConnection]);

    return useMemo(() => ({
        contextMenu,
        setContextMenu,
        pendingConnection,
        setPendingConnection,
        connectPins,
        onCanvasPointerDown,
        onNodePointerDown,
        onPinPointerDown
    }), [
        contextMenu,
        setContextMenu,
        pendingConnection,
        setPendingConnection,
        connectPins,
        onCanvasPointerDown,
        onNodePointerDown,
        onPinPointerDown,
    ]);
}
