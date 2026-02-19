import { useState, useEffect, useCallback, useLayoutEffect, useMemo } from "react";
import { Pin } from "@/shared/types/domain";
import { useViewportStore } from "@/features/core/viewport";
import { useGraphDataStore } from "@/features/core/dataStore";
import { DEFAULT_VIEWPORT } from "@/app/appConfig/default";

const NODE_WIDTH = 300;
const NODE_HEIGHT = 300;
const CULLING_PADDING_FACTOR = 200;

/**
 * Viewport culling, wheel zoom, pin offsets, and coordinate helpers for Canvas.
 * Extracted from Canvas.tsx - view should only consume this hook.
 */
export function useCanvasViewport(
  canvasRef: React.RefObject<HTMLDivElement | null>,
  groupId: string,
  activeTabId: string | null,
  nodes: { id: string; position: { x: number; y: number }; inputs: Pin[]; outputs: Pin[] }[],
  scale: number,
  gestureType: string | null,
  setCanvas: (updater: { scale?: number; x?: number; y?: number } | ((prev: any) => any), targetGroupId?: string) => void,
  dragDelta?: { x: number; y: number } | null,
  dragNodeIds?: Set<string>
) {
  const [visibleNodeIds, setVisibleNodes] = useState<Set<string>>(new Set());
  const [pinOffsets, setPinOffsets] = useState<Record<string, { x: number; y: number }>>({});

  const updateVisibleNodes = useCallback(() => {
    const el = canvasRef.current;
    if (!el) return;

    const rect = el.getBoundingClientRect();
    const viewport = useViewportStore.getState().viewports[groupId] || DEFAULT_VIEWPORT;

    const padding = CULLING_PADDING_FACTOR / viewport.scale;
    const worldViewLeft = -viewport.x / viewport.scale - padding;
    const worldViewTop = -viewport.y / viewport.scale - padding;
    const worldViewRight = (rect.width - viewport.x) / viewport.scale + padding;
    const worldViewBottom = (rect.height - viewport.y) / viewport.scale + padding;

    // 直接从 store 读取节点位置，避免 deserializeGraph
    const store = useGraphDataStore.getState();
    const nodeIds = activeTabId ? store.graphNodes[activeTabId] ?? [] : [];

    const visible = new Set<string>();
    for (const nid of nodeIds) {
      const node = store.nodes[nid];
      if (!node?.position) continue;
      const isVisible =
        node.position.x + NODE_WIDTH > worldViewLeft &&
        node.position.x < worldViewRight &&
        node.position.y + NODE_HEIGHT > worldViewTop &&
        node.position.y < worldViewBottom;
      if (isVisible) visible.add(nid);
    }
    setVisibleNodes(visible);
  }, [canvasRef, groupId, activeTabId]);

  useEffect(() => {
    updateVisibleNodes();
  }, [scale, nodes, updateVisibleNodes]);

  useEffect(() => {
    if (!gestureType) updateVisibleNodes();
  }, [gestureType, updateVisibleNodes]);

  const handleWheel = useCallback(
    (e: WheelEvent) => {
      const target = e.target as HTMLElement;
      if (
        target.closest(".menubar-container") ||
        target.closest(".sidebar-container") ||
        target.closest(".menu-container") ||
        target.closest(".hud-container")
      ) {
        return;
      }

      const canvasEl = canvasRef.current;
      if (!canvasEl) return;
      const rect = canvasEl.getBoundingClientRect();
      if (
        e.clientX < rect.left ||
        e.clientX > rect.right ||
        e.clientY < rect.top ||
        e.clientY > rect.bottom
      ) {
        return;
      }

      e.preventDefault();
      const factor = 0.001;
      const currentCanvas = useViewportStore.getState().viewports[groupId] || { x: 0, y: 0, scale: 1 };
      const nextScale = Math.min(Math.max(currentCanvas.scale * (1 - e.deltaY * factor), 0.2), 4);

      const mouseX = e.clientX - rect.left;
      const mouseY = e.clientY - rect.top;
      const worldX = (mouseX - currentCanvas.x) / currentCanvas.scale;
      const worldY = (mouseY - currentCanvas.y) / currentCanvas.scale;

      setCanvas({
        scale: nextScale,
        x: mouseX - worldX * nextScale,
        y: mouseY - worldY * nextScale,
      });
    },
    [canvasRef, setCanvas, groupId]
  );

  useEffect(() => {
    const canvasEl = canvasRef.current;
    if (!canvasEl) return;
    window.addEventListener("wheel", handleWheel, { passive: false, capture: true });
    return () => window.removeEventListener("wheel", handleWheel, { capture: true });
  }, [handleWheel]);

  // pin→nodeId 映射，只在 nodes 变化时重建
  const pinNodeIdMap = useMemo(() => {
    const map = new Map<string, string>();
    nodes.forEach((node) => {
      node.inputs.forEach((pin: Pin) => map.set(pin.id, node.id));
      node.outputs.forEach((pin: Pin) => map.set(pin.id, node.id));
    });
    return map;
  }, [nodes]);

  // node 位置映射，只在 nodes 变化时重建
  const nodePositionMap = useMemo(() => {
    const map = new Map<string, { x: number; y: number }>();
    nodes.forEach((node) => map.set(node.id, node.position));
    return map;
  }, [nodes]);

  useLayoutEffect(() => {
    const root = canvasRef.current;
    if (!root) return;
    const nextOffsets: Record<string, { x: number; y: number }> = {};

    nodes.forEach((node) => {
      const nodeEl = root.querySelector(`[data-node-id="${node.id}"]`);
      if (!nodeEl) return;

      const nodeRect = nodeEl.getBoundingClientRect();
      const pins = nodeEl.querySelectorAll<HTMLElement>("[data-pin-id]");

      pins.forEach((pinEl) => {
        const pinId = pinEl.dataset.pinId;
        if (!pinId) return;
        const circleEl = pinEl.querySelector(".pin-circle");
        const targetEl = circleEl || pinEl;
        const rect = targetEl.getBoundingClientRect();
        nextOffsets[pinId] = {
          x: (rect.left + rect.width / 2 - nodeRect.left) / scale,
          y: (rect.top + rect.height / 2 - nodeRect.top) / scale,
        };
      });
    });

    setPinOffsets((prev) => {
      const currentKeys = Object.keys(nextOffsets);
      const prevKeys = Object.keys(prev);
      if (currentKeys.length === prevKeys.length) {
        const isSame = currentKeys.every(
          (k) =>
            prev[k] &&
            Math.abs(prev[k].x - nextOffsets[k].x) < 0.1 &&
            Math.abs(prev[k].y - nextOffsets[k].y) < 0.1
        );
        if (isSame) return prev;
      }
      return nextOffsets;
    });
  }, [canvasRef, scale, visibleNodeIds, nodes]);

  // getPinWorldPos: 使用 useMemo 缓存的 Map，O(1) 查找，不再调用 deserializeGraph
  const getPinWorldPos = useCallback(
    (pinId: string) => {
      const nodeId = pinNodeIdMap.get(pinId);
      if (!nodeId) return null;
      const position = nodePositionMap.get(nodeId);
      const offset = pinOffsets[pinId];
      if (!position || !offset) return null;
      const ddx = dragDelta && dragNodeIds?.has(nodeId) ? dragDelta.x : 0;
      const ddy = dragDelta && dragNodeIds?.has(nodeId) ? dragDelta.y : 0;
      return {
        x: position.x + offset.x + ddx,
        y: position.y + offset.y + ddy,
      };
    },
    [pinNodeIdMap, nodePositionMap, pinOffsets, dragDelta, dragNodeIds]
  );

  const getCanvasLocalPoint = useCallback(
    (clientX: number, clientY: number) => {
      const root = canvasRef.current;
      if (!root) return { x: 0, y: 0 };
      const rect = root.getBoundingClientRect();
      const currentCanvas = useViewportStore.getState().viewports[groupId] || { x: 0, y: 0, scale: 1 };
      return {
        x: (clientX - rect.left - currentCanvas.x) / currentCanvas.scale,
        y: (clientY - rect.top - currentCanvas.y) / currentCanvas.scale,
      };
    },
    [canvasRef, groupId]
  );

  return {
    visibleNodeIds,
    pinOffsets,
    getPinWorldPos,
    getCanvasLocalPoint,
  };
}
