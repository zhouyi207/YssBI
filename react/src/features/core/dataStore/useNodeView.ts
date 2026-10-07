import { useMemo } from "react";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { createNodeViewSelector, type UINode } from "./nodeView";

/** Subscribe to content only; the canvas owns node coordinates and interaction decoration. */
export function useNodeView(nodeId: string, graphPath?: string): UINode | null {
  const select = useMemo(() => createNodeViewSelector(nodeId, graphPath), [nodeId, graphPath]);
  return useResourceStore(select);
}
