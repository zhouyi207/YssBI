import { useState, useEffect, useCallback, useLayoutEffect, useMemo } from "react";
import { Pin } from "@/shared/types/domain";
import { useViewportStore } from "@/features/core/viewport";
import { useGraphDataStore } from "@/features/core/dataStore";
import { DEFAULT_VIEWPORT, NODE_WIDTH, NODE_HEIGHT, CULLING_PADDING_FACTOR } from "@/app/appConfig/default";

/** 线段 (x1,y1)-(x2,y2) 与矩形 [left,top,right,bottom] 是否相交 */
function segmentIntersectsRect(
  x1: number, y1: number, x2: number, y2: number,
  left: number, top: number, right: number, bottom: number
): boolean {
  if (x1 >= left && x1 <= right && y1 >= top && y1 <= bottom) return true;
  if (x2 >= left && x2 <= right && y2 >= top && y2 <= bottom) return true;
  const dx = x2 - x1, dy = y2 - y1;
  const t = (p: number, q: number, d: number) => (d === 0 ? (q === p ? 0 : NaN) : (q - p) / d);
  const ts = [
    t(x1, left, dx), t(x1, right, dx), t(y1, top, dy), t(y1, bottom, dy)
  ].filter((v) => !Number.isNaN(v) && v >= 0 && v <= 1);
  for (const v of ts) {
    const x = x1 + v * dx, y = y1 + v * dy;
    if (x >= left && x <= right && y >= top && y <= bottom) return true;
  }
  return false;
}

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

    // 扩展可见集：确保边线能正确绘制
    // 1) 与可见节点有连边的节点
    // 2) 边线段穿过视口的节点（长边在中间时两端都不可见，但边应显示）
    const pinToNode = new Map<string, string>();
    for (const nid of nodeIds) {
      for (const pid of store.nodePins[nid] ?? []) {
        pinToNode.set(pid, nid);
      }
    }
    const nodeCenter = (nid: string) => {
      const node = store.nodes[nid];
      if (!node?.position) return null;
      return {
        x: node.position.x + NODE_WIDTH / 2,
        y: node.position.y + NODE_HEIGHT / 2,
      };
    };
    for (const conn of Object.values(store.connections)) {
      const fromNode = pinToNode.get(conn.from);
      const toNode = pinToNode.get(conn.to);
      if (!fromNode || !toNode) continue;
      const addBoth = visible.has(fromNode) || visible.has(toNode) || (() => {
        const p1 = nodeCenter(fromNode);
        const p2 = nodeCenter(toNode);
        if (!p1 || !p2) return false;
        return segmentIntersectsRect(
          p1.x, p1.y, p2.x, p2.y,
          worldViewLeft, worldViewTop, worldViewRight, worldViewBottom
        );
      })();
      if (addBoth) {
        visible.add(fromNode);
        visible.add(toNode);
      }
    }

    setVisibleNodes(visible);
  }, [canvasRef, groupId, activeTabId]);

  // 订阅 viewport 变化，画布平移/缩放时连续更新可见集（而非拖拽结束才更新）
  const viewport = useViewportStore((state) => state.viewports[groupId] || DEFAULT_VIEWPORT);
  useEffect(() => {
    updateVisibleNodes();
  }, [scale, nodes, viewport.x, viewport.y, viewport.scale, updateVisibleNodes]);

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
      const nextScale = Math.min(Math.max(currentCanvas.scale * (1 - e.deltaY * factor), 0.2), 1.2);

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

  // getPinWorldPos: 使用 DOM 测量的 pin 偏移，可见集已包含连边节点，故所有需绘边的 pin 均有测量值
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
