import { useState, useEffect, useCallback, useLayoutEffect, useRef } from "react";
import { useShallow } from "zustand/react/shallow";
import { useViewportStore } from "@/features/core/viewport";
import { useGraphDataStore } from "@/features/core/dataStore";
import { getDragPreview } from "@/features/core/canvas/dragPreview";
import { DEFAULT_VIEWPORT, NODE_WIDTH, NODE_HEIGHT, CULLING_PADDING_FACTOR } from "@/app/appConfig/default";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import { ProjectService } from "@/services/project/projectService";
import { clamp } from "@/shared/utils";
import { resolvePinOffsetWaiters } from "@/features/core/canvas/pinOffsetWaiter";

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
 */
export function useCanvasViewport(
  canvasRef: React.RefObject<HTMLDivElement | null>,
  graphId: string | null,
  scale: number,
  gestureType: string | null,
  setCanvas: (updater: { scale?: number; x?: number; y?: number } | ((prev: any) => any), targetGraphId?: string) => void,
) {
  const [visibleNodeIds, setVisibleNodes] = useState<Set<string>>(new Set());
  const [pinOffsets, setPinOffsets] = useState<Record<string, { x: number; y: number }>>({});
  const wheelPersistTimerRef = useRef<number | null>(null);
  const cullingTimerRef = useRef<number | null>(null);

  // 轻量 store 订阅：仅在「位置 / pin 归属 / 连接」实际变化时更新，
  // 不再依赖整图反序列化出的 nodes 数组。
  const nodePositionMap = useGraphDataStore(
    useShallow((s) => {
      const ids = graphId ? s.graphNodes[graphId] ?? [] : [];
      const m: Record<string, { x: number; y: number }> = {};
      for (const id of ids) {
        const n = graphId ? s.getGraphNode(graphId, id) : undefined;
        if (n?.position) m[id] = n.position;
      }
      return m;
    }),
  );

  const pinNodeIdMap = useGraphDataStore(
    useShallow((s) => {
      const ids = graphId ? s.graphNodes[graphId] ?? [] : [];
      const m: Record<string, string> = {};
      for (const nid of ids) {
        if (!graphId) continue;
        for (const pid of s.getGraphNodePins(graphId, nid)) m[pid] = nid;
      }
      return m;
    }),
  );

  // 连接表引用：连接增删时重算 culling（保持「连线另一端节点」可见性）。
  const connectionsRef = useGraphDataStore(
    useShallow((s) => (graphId ? s.getGraphConnections(graphId) : [])),
  );

  const persistViewport = useCallback(() => {
    if (!graphId) return;
    const viewport = useViewportStore.getState().viewports[graphId];
    if (!viewport) return;
    ProjectService.updateCanvas(graphId, viewport).catch(() => {});
  }, [graphId]);

  const scheduleViewportPersist = useCallback(() => {
    if (wheelPersistTimerRef.current !== null) {
      window.clearTimeout(wheelPersistTimerRef.current);
    }
    wheelPersistTimerRef.current = window.setTimeout(() => {
      wheelPersistTimerRef.current = null;
      persistViewport();
    }, 300);
  }, [persistViewport]);

  const updateVisibleNodes = useCallback(() => {
    const el = canvasRef.current;
    if (!el || !graphId) return;

    const rect = el.getBoundingClientRect();
    const viewport = useViewportStore.getState().viewports[graphId] || DEFAULT_VIEWPORT;

    const padding = CULLING_PADDING_FACTOR / viewport.scale;
    const worldViewLeft = -viewport.x / viewport.scale - padding;
    const worldViewTop = -viewport.y / viewport.scale - padding;
    const worldViewRight = (rect.width - viewport.x) / viewport.scale + padding;
    const worldViewBottom = (rect.height - viewport.y) / viewport.scale + padding;

    const store = useGraphDataStore.getState();
    const nodeIds = store.graphNodes[graphId] ?? [];

    const visible = new Set<string>();
    for (const nid of nodeIds) {
      const node = store.getGraphNode(graphId, nid);
      if (!node?.position) continue;
      const isVisible =
        node.position.x + NODE_WIDTH > worldViewLeft &&
        node.position.x < worldViewRight &&
        node.position.y + NODE_HEIGHT > worldViewTop &&
        node.position.y < worldViewBottom;
      if (isVisible) visible.add(nid);
    }

    const pinToNode = new Map<string, string>();
    for (const nid of nodeIds) {
      for (const pid of store.getGraphNodePins(graphId, nid)) {
        pinToNode.set(pid, nid);
      }
    }
    const nodeCenter = (nid: string) => {
      const node = store.getGraphNode(graphId, nid);
      if (!node?.position) return null;
      return {
        x: node.position.x + NODE_WIDTH / 2,
        y: node.position.y + NODE_HEIGHT / 2,
      };
    };
    for (const conn of store.getGraphConnections(graphId)) {
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
  }, [canvasRef, graphId]);

  useEffect(() => {
    updateVisibleNodes();
  }, [scale, nodePositionMap, connectionsRef, updateVisibleNodes]);

  useEffect(() => {
    if (!gestureType) updateVisibleNodes();
  }, [gestureType, updateVisibleNodes]);

  useEffect(() => {
    if (gestureType !== "pan" || !graphId) return;

    return useViewportStore.subscribe((state, prevState) => {
      if (state.viewports[graphId] === prevState.viewports[graphId]) return;
      if (cullingTimerRef.current !== null) return;
      cullingTimerRef.current = window.setTimeout(() => {
        cullingTimerRef.current = null;
        updateVisibleNodes();
      }, 120);
    });
  }, [gestureType, graphId, updateVisibleNodes]);

  const handleWheel = useCallback(
    (e: WheelEvent) => {
      if (!graphId) return;

      const target = e.target as HTMLElement;
      if (
        target.closest(".menubar-container") ||
        target.closest(".sidebar-container") ||
        target.closest(".menu-container")
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

      const currentCanvas = useViewportStore.getState().viewports[graphId] || DEFAULT_VIEWPORT;
      if (e.ctrlKey || e.metaKey) {
        const mouseX = e.clientX - rect.left;
        const mouseY = e.clientY - rect.top;
        const factor = Math.pow(1.1, -e.deltaY / 100);
        const nextScale = clamp(currentCanvas.scale * factor, 0.1, 5);
        const worldX = (mouseX - currentCanvas.x) / currentCanvas.scale;
        const worldY = (mouseY - currentCanvas.y) / currentCanvas.scale;

        setCanvas({
          scale: nextScale,
          x: mouseX - worldX * nextScale,
          y: mouseY - worldY * nextScale,
        });
      } else {
        const panX = e.shiftKey && e.deltaX === 0 ? e.deltaY : e.deltaX;
        const panY = e.shiftKey && e.deltaX === 0 ? 0 : e.deltaY;
        setCanvas((prev: { x: number; y: number; scale: number }) => ({
          ...prev,
          x: prev.x - panX,
          y: prev.y - panY,
        }));
      }

      scheduleViewportPersist();
    },
    [canvasRef, graphId, setCanvas, scheduleViewportPersist]
  );

  useEffect(() => {
    const canvasEl = canvasRef.current;
    if (!canvasEl) return;
    return addGlobalEventListener(window, "wheel", handleWheel, { passive: false, capture: true });
  }, [handleWheel]);

  useEffect(() => {
    return () => {
      if (wheelPersistTimerRef.current !== null) window.clearTimeout(wheelPersistTimerRef.current);
      if (cullingTimerRef.current !== null) window.clearTimeout(cullingTimerRef.current);
    };
  }, []);

  const [nodeResizeVersion, setNodeResizeVersion] = useState(0);
  const resizeRafRef = useRef(0);

  useEffect(() => {
    const root = canvasRef.current;
    if (!root) return;

    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(resizeRafRef.current);
      resizeRafRef.current = requestAnimationFrame(() => {
        setNodeResizeVersion((v) => v + 1);
      });
    });

    visibleNodeIds.forEach((nodeId) => {
      const el = root.querySelector(`[data-node-id="${nodeId}"]`);
      if (el) observer.observe(el);
    });

    return () => {
      cancelAnimationFrame(resizeRafRef.current);
      observer.disconnect();
    };
  }, [canvasRef, visibleNodeIds]);

  useLayoutEffect(() => {
    const root = canvasRef.current;
    if (!root) return;
    const nextOffsets: Record<string, { x: number; y: number }> = {};

    visibleNodeIds.forEach((nodeId) => {
      const nodeEl = root.querySelector(`[data-node-id="${nodeId}"]`);
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

    // 兑现等待该 pin 偏移的创建流程（从 pin 拖拽建节点后的位置对齐）
    if (graphId) resolvePinOffsetWaiters(graphId, nextOffsets);
  }, [canvasRef, scale, visibleNodeIds, nodeResizeVersion, graphId]);

  const getPinWorldPos = useCallback(
    (pinId: string) => {
      const nodeId = pinNodeIdMap[pinId];
      if (!nodeId) return null;
      const position = nodePositionMap[nodeId];
      const offset = pinOffsets[pinId];
      if (!position || !offset) return null;
      const preview = getDragPreview();
      const ddx = preview.active && preview.dragNodeIds.has(nodeId) ? preview.dragDelta.x : 0;
      const ddy = preview.active && preview.dragNodeIds.has(nodeId) ? preview.dragDelta.y : 0;
      return {
        x: position.x + offset.x + ddx,
        y: position.y + offset.y + ddy,
      };
    },
    [pinNodeIdMap, nodePositionMap, pinOffsets]
  );

  const getCanvasLocalPoint = useCallback(
    (clientX: number, clientY: number) => {
      const root = canvasRef.current;
      if (!root || !graphId) return { x: 0, y: 0 };
      const rect = root.getBoundingClientRect();
      const currentCanvas = useViewportStore.getState().viewports[graphId] || DEFAULT_VIEWPORT;
      return {
        x: (clientX - rect.left - currentCanvas.x) / currentCanvas.scale,
        y: (clientY - rect.top - currentCanvas.y) / currentCanvas.scale,
      };
    },
    [canvasRef, graphId]
  );

  return {
    visibleNodeIds,
    pinOffsets,
    getPinWorldPos,
    getCanvasLocalPoint,
  };
}
