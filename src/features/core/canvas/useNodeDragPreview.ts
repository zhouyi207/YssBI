import { useEffect, useRef } from "react";
import { useGraphDataStore } from "@/features/core/dataStore";
import { useGraphInteractionStore } from "@/features/core/graphInteraction";
import { getDragPreview, subscribeDragPreview } from "./dragPreview";

/**
 * Applies node drag offset imperatively during pointer drag so React nodes
 * do not re-render every frame. Scoped to one editor group pane.
 */
export function useNodeDragPreview(
  canvasElementRef: React.RefObject<HTMLDivElement | null>,
  groupId: string | null,
  graphPath: string | null,
): void {
  const lastDraggedRef = useRef<Set<string>>(new Set());

  useEffect(() => {
    const root = canvasElementRef.current;
    if (!root || !graphPath || !groupId) return;

    const scope = { graphPath, groupId };
    const apply = () => {
      const preview = getDragPreview(scope);
      const appliesHere = preview.active && preview.groupId === groupId;
      const store = useGraphDataStore.getState();

      if (!appliesHere) {
        for (const nodeId of lastDraggedRef.current) {
          const el = root.querySelector(`[data-node-id="${nodeId}"]`) as HTMLElement | null;
          const committed = store.getGraphNode(graphPath, nodeId)?.position;
          const pos =
            useGraphInteractionStore.getState().positionOverrides[graphPath]?.[nodeId] ?? committed;
          if (!el || !pos) continue;
          el.style.transform = `translate3d(${pos.x}px, ${pos.y}px, 0)`;
        }
        lastDraggedRef.current = new Set();
        return;
      }

      lastDraggedRef.current = new Set(preview.dragNodeIds);
      for (const nodeId of preview.dragNodeIds) {
        const el = root.querySelector(`[data-node-id="${nodeId}"]`) as HTMLElement | null;
        const pos =
          useGraphInteractionStore.getState().positionOverrides[graphPath]?.[nodeId] ??
          store.getGraphNode(graphPath, nodeId)?.position;
        if (!el || !pos) continue;
        el.style.transform = `translate3d(${pos.x}px, ${pos.y}px, 0)`;
      }
    };

    apply();
    return subscribeDragPreview(scope, apply);
  }, [canvasElementRef, groupId, graphPath]);
}
