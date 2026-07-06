/**
 * Viewport culling, wheel zoom, pin offsets, and coordinate helpers for Canvas.
 */
import { useState, useEffect, useCallback, useLayoutEffect, useRef } from 'react';
import { useShallow } from 'zustand/react/shallow';
import {
  attachViewportWheel,
  getViewport,
  subscribeToViewport,
} from '@/features/core/viewport';
import { useGraphDataStore } from '@/features/core/dataStore';
import { getDragPreview } from '@/features/core/canvas/dragPreview';
import { NODE_WIDTH, NODE_HEIGHT, CULLING_PADDING_FACTOR } from '@/app/appConfig/default';
import { resolvePinOffsetWaiters } from '@/features/core/canvas/pinOffsetWaiter';

/** 线段 (x1,y1)-(x2,y2) 与矩形 [left,top,right,bottom] 是否相交 */
function segmentIntersectsRect(
  x1: number, y1: number, x2: number, y2: number,
  left: number, top: number, right: number, bottom: number,
): boolean {
  if (x1 >= left && x1 <= right && y1 >= top && y1 <= bottom) return true;
  if (x2 >= left && x2 <= right && y2 >= top && y2 <= bottom) return true;
  const dx = x2 - x1;
  const dy = y2 - y1;
  const t = (p: number, q: number, d: number) => (d === 0 ? (q === p ? 0 : NaN) : (q - p) / d);
  const ts = [
    t(x1, left, dx), t(x1, right, dx), t(y1, top, dy), t(y1, bottom, dy),
  ].filter((v) => !Number.isNaN(v) && v >= 0 && v <= 1);
  for (const v of ts) {
    const x = x1 + v * dx;
    const y = y1 + v * dy;
    if (x >= left && x <= right && y >= top && y <= bottom) return true;
  }
  return false;
}

export function useCanvasViewport(
  canvasElementRef: React.RefObject<HTMLDivElement | null>,
  graphId: string | null,
  gestureType: string | null,
) {
  const [visibleNodeIds, setVisibleNodes] = useState<Set<string>>(new Set());
  const [pinOffsets, setPinOffsets] = useState<Record<string, { x: number; y: number }>>({});
  const cullingTimerRef = useRef<number | null>(null);

  const nodePositionMap = useGraphDataStore(
    useShallow((s) => {
      const ids = graphId ? s.getGraphNodeIds(graphId) : [];
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
      const ids = graphId ? s.getGraphNodeIds(graphId) : [];
      const m: Record<string, string> = {};
      for (const nid of ids) {
        if (!graphId) continue;
        for (const pid of s.getGraphNodePins(graphId, nid)) m[pid] = nid;
      }
      return m;
    }),
  );

  const connectionsRef = useGraphDataStore(
    useShallow((s) => (graphId ? s.getGraphConnections(graphId) : [])),
  );

  useEffect(() => {
    const canvasEl = canvasElementRef.current;
    if (!canvasEl || !graphId) return;
    return attachViewportWheel(canvasEl, graphId);
  }, [canvasElementRef, graphId]);

  const updateVisibleNodes = useCallback(() => {
    const el = canvasElementRef.current;
    if (!el || !graphId) return;

    const rect = el.getBoundingClientRect();
    const viewport = getViewport(graphId);

    const padding = CULLING_PADDING_FACTOR / viewport.scale;
    const worldViewLeft = -viewport.x / viewport.scale - padding;
    const worldViewTop = -viewport.y / viewport.scale - padding;
    const worldViewRight = (rect.width - viewport.x) / viewport.scale + padding;
    const worldViewBottom = (rect.height - viewport.y) / viewport.scale + padding;

    const store = useGraphDataStore.getState();
    const nodeIds = store.getGraphNodeIds(graphId);

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
          worldViewLeft, worldViewTop, worldViewRight, worldViewBottom,
        );
      })();
      if (addBoth) {
        visible.add(fromNode);
        visible.add(toNode);
      }
    }

    setVisibleNodes(visible);
  }, [canvasElementRef, graphId]);

  const scheduleCullingUpdate = useCallback(() => {
    if (cullingTimerRef.current !== null) return;
    cullingTimerRef.current = window.setTimeout(() => {
      cullingTimerRef.current = null;
      updateVisibleNodes();
    }, 120);
  }, [updateVisibleNodes]);

  useEffect(() => {
    updateVisibleNodes();
  }, [nodePositionMap, connectionsRef, updateVisibleNodes]);

  useEffect(() => {
    if (!graphId) return;
    return subscribeToViewport(graphId, scheduleCullingUpdate);
  }, [graphId, scheduleCullingUpdate]);

  useEffect(() => {
    if (!gestureType) updateVisibleNodes();
  }, [gestureType, updateVisibleNodes]);

  useEffect(() => {
    return () => {
      if (cullingTimerRef.current !== null) window.clearTimeout(cullingTimerRef.current);
    };
  }, []);

  const [nodeResizeVersion, setNodeResizeVersion] = useState(0);
  const resizeRafRef = useRef(0);

  useEffect(() => {
    const root = canvasElementRef.current;
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
  }, [canvasElementRef, visibleNodeIds]);

  useLayoutEffect(() => {
    const root = canvasElementRef.current;
    if (!root || !graphId) return;

    const scale = getViewport(graphId).scale;
    const nextOffsets: Record<string, { x: number; y: number }> = {};

    visibleNodeIds.forEach((nodeId) => {
      const nodeEl = root.querySelector(`[data-node-id="${nodeId}"]`);
      if (!nodeEl) return;

      const nodeRect = nodeEl.getBoundingClientRect();
      const pins = nodeEl.querySelectorAll<HTMLElement>('[data-pin-id]');

      pins.forEach((pinEl) => {
        const pinId = pinEl.dataset.pinId;
        if (!pinId) return;
        const circleEl = pinEl.querySelector('.pin-circle');
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
            prev[k]
            && Math.abs(prev[k].x - nextOffsets[k].x) < 0.1
            && Math.abs(prev[k].y - nextOffsets[k].y) < 0.1,
        );
        if (isSame) return prev;
      }
      return nextOffsets;
    });

    resolvePinOffsetWaiters(graphId, nextOffsets);
  }, [canvasElementRef, visibleNodeIds, nodeResizeVersion, graphId]);

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
    [pinNodeIdMap, nodePositionMap, pinOffsets],
  );

  const getCanvasLocalPoint = useCallback(
    (clientX: number, clientY: number) => {
      const root = canvasElementRef.current;
      if (!root || !graphId) return { x: 0, y: 0 };
      const rect = root.getBoundingClientRect();
      const viewport = getViewport(graphId);
      return {
        x: (clientX - rect.left - viewport.x) / viewport.scale,
        y: (clientY - rect.top - viewport.y) / viewport.scale,
      };
    },
    [canvasElementRef, graphId],
  );

  return {
    visibleNodeIds,
    pinOffsets,
    getPinWorldPos,
    getCanvasLocalPoint,
  };
}
