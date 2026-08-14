/**
 * Pin offsets and coordinate helpers for Canvas.
 */
import { useState, useEffect, useCallback, useLayoutEffect, useRef } from 'react';
import { useShallow } from 'zustand/react/shallow';
import {
  getViewport,
  editorViewportScope,
} from '@/features/core/viewport';
import { useGraphDataStore } from '@/features/core/dataStore';
import { useGraphInteractionStore } from '@/features/core/graphInteraction';
import { resolvePinOffsetWaiters } from '@/features/core/canvas/pinOffsetWaiter';


export function useCanvasViewport(
  canvasElementRef: React.RefObject<HTMLDivElement | null>,
  groupId: string | null,
  graphPath: string | null,
) {
  const viewportScope =
    groupId && graphPath ? editorViewportScope(groupId, graphPath) : null;
  const [pinOffsets, setPinOffsets] = useState<Record<string, { x: number; y: number }>>({});

  const graphNodeIds = useGraphDataStore(
    useShallow((s) => (graphPath ? s.getGraphNodeIds(graphPath) : [])),
  );

  const nodePositionMap = useGraphDataStore(
    useShallow((s) => {
      const ids = graphPath ? s.getGraphNodeIds(graphPath) : [];
      const m: Record<string, { x: number; y: number }> = {};
      for (const id of ids) {
        const n = graphPath ? s.getGraphNode(graphPath, id) : undefined;
        if (n?.position) m[id] = n.position;
      }
      return m;
    }),
  );

  const pinNodeIdMap = useGraphDataStore(
    useShallow((s) => {
      const ids = graphPath ? s.getGraphNodeIds(graphPath) : [];
      const m: Record<string, string> = {};
      for (const nid of ids) {
        if (!graphPath) continue;
        for (const pid of s.getGraphNodePins(graphPath, nid)) m[pid] = nid;
      }
      return m;
    }),
  );

  const [nodeResizeVersion, setNodeResizeVersion] = useState(0);
  const resizeRafRef = useRef(0);

  useEffect(() => {
    const root = canvasElementRef.current;
    if (!root) return;

    const bumpResizeVersion = () => {
      setNodeResizeVersion((v) => v + 1);
    };
    const observer = new ResizeObserver(() => {
      cancelAnimationFrame(resizeRafRef.current);
      resizeRafRef.current = requestAnimationFrame(bumpResizeVersion);
    });

    for (const nodeId of graphNodeIds) {
      const el = root.querySelector(`[data-node-id="${nodeId}"]`);
      if (el) observer.observe(el);
    }

    return () => {
      cancelAnimationFrame(resizeRafRef.current);
      observer.disconnect();
    };
  }, [canvasElementRef, graphNodeIds]);

  useLayoutEffect(() => {
    const root = canvasElementRef.current;
    if (!root || !graphPath) return;

    const rootRect = root.getBoundingClientRect();
    if (rootRect.width <= 0 || rootRect.height <= 0) return;

    const scale = viewportScope ? getViewport(viewportScope).scale : 1;
    const nextOffsets: Record<string, { x: number; y: number }> = {};
    let hasUnmeasurablePin = false;

    for (const nodeId of graphNodeIds) {
      const nodeEl = root.querySelector(`[data-node-id="${nodeId}"]`);
      if (!nodeEl) continue;

      const nodeRect = nodeEl.getBoundingClientRect();
      if (nodeRect.width <= 0 || nodeRect.height <= 0) {
        hasUnmeasurablePin = true;
        continue;
      }
      const pins = nodeEl.querySelectorAll<HTMLElement>('[data-pin-id]');

      pins.forEach((pinEl) => {
        const pinId = pinEl.dataset.pinId;
        if (!pinId) return;
        const circleEl = pinEl.querySelector('.pin-circle');
        const targetEl = circleEl || pinEl;
        const rect = targetEl.getBoundingClientRect();
        if (rect.width <= 0 || rect.height <= 0) {
          hasUnmeasurablePin = true;
          return;
        }
        nextOffsets[pinId] = {
          x: (rect.left + rect.width / 2 - nodeRect.left) / scale,
          y: (rect.top + rect.height / 2 - nodeRect.top) / scale,
        };
      });
    }

    if (hasUnmeasurablePin) return;

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

    resolvePinOffsetWaiters(graphPath, nextOffsets);
  }, [canvasElementRef, graphNodeIds, nodeResizeVersion, graphPath, viewportScope]);

  const getPinWorldPos = useCallback(
    (pinId: string) => {
      const nodeId = pinNodeIdMap[pinId];
      if (!nodeId) return null;
      const position = graphPath
        ? useGraphInteractionStore.getState().positionOverrides[graphPath]?.[nodeId]
          ?? nodePositionMap[nodeId]
        : nodePositionMap[nodeId];
      const offset = pinOffsets[pinId];
      if (!position || !offset) return null;
      return {
        x: position.x + offset.x,
        y: position.y + offset.y,
      };
    },
    [graphPath, pinNodeIdMap, nodePositionMap, pinOffsets],
  );

  const getCanvasLocalPoint = useCallback(
    (clientX: number, clientY: number) => {
      const root = canvasElementRef.current;
      if (!root || !graphPath) return { x: 0, y: 0 };
      const rect = root.getBoundingClientRect();
      const viewport = viewportScope ? getViewport(viewportScope) : { x: 0, y: 0, scale: 1 };
      return {
        x: (clientX - rect.left - viewport.x) / viewport.scale,
        y: (clientY - rect.top - viewport.y) / viewport.scale,
      };
    },
    [canvasElementRef, graphPath, viewportScope],
  );

  return {
    getPinWorldPos,
    getCanvasLocalPoint,
  };
}
