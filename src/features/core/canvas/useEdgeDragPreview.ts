import { useEffect, useRef } from 'react';
import type { EdgeData } from '@/views/EditorView/Canvas/core/EdgesOverlay';
import { computeEdgePath } from './edgePath';
import { getDragPreview, subscribeDragPreview } from './dragPreview';

function edgeTouchesDrag(edge: EdgeData, dragNodeIds: ReadonlySet<string>): boolean {
  return dragNodeIds.has(edge.sourceNodeId)
    || (edge.targetNodeId != null && dragNodeIds.has(edge.targetNodeId));
}

function updateEdgePaths(
  svg: SVGSVGElement,
  edges: EdgeData[],
  getPinWorldPos: (pinId: string) => { x: number; y: number } | null,
  onlyDirty: boolean,
  dragNodeIds: ReadonlySet<string>,
): void {
  for (const edge of edges) {
    if (onlyDirty && !edgeTouchesDrag(edge, dragNodeIds)) continue;

    const start = getPinWorldPos(edge.fromPinId);
    const end = getPinWorldPos(edge.toPinId);
    if (!start || !end) continue;

    const pathData = computeEdgePath(start.x, start.y, end.x, end.y);
    const group = svg.querySelector(`[data-edge-id="${edge.id}"]`);
    if (!group) continue;

    group.querySelectorAll('path').forEach((pathEl) => {
      pathEl.setAttribute('d', pathData);
    });
  }
}

/**
 * Imperatively updates only edges attached to dragged nodes during drag.
 */
export function useEdgeDragPreview(
  svgRef: React.RefObject<SVGSVGElement | null>,
  edges: EdgeData[],
  getPinWorldPos: (pinId: string) => { x: number; y: number } | null,
  scope: { graphPath: string; groupId: string },
): void {
  const edgesRef = useRef(edges);
  const getPinWorldPosRef = useRef(getPinWorldPos);
  const lastDirtyRef = useRef<Set<string>>(new Set());

  edgesRef.current = edges;
  getPinWorldPosRef.current = getPinWorldPos;

  useEffect(() => {
    const sync = () => {
      const svg = svgRef.current;
      if (!svg) return;

      const preview = getDragPreview(scope);
      const getPos = getPinWorldPosRef.current;
      const currentEdges = edgesRef.current;

      if (!preview.active) {
        if (lastDirtyRef.current.size === 0) return;
        updateEdgePaths(svg, currentEdges, getPos, true, lastDirtyRef.current);
        lastDirtyRef.current = new Set();
        return;
      }

      lastDirtyRef.current = new Set(preview.dragNodeIds);
      updateEdgePaths(svg, currentEdges, getPos, true, preview.dragNodeIds);
    };

    return subscribeDragPreview(sync);
  }, [svgRef, scope.graphPath, scope.groupId]);
}
