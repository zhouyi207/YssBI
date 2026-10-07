import { useResourceStore } from "@/features/core/resource/resourceStore";

/** Missing projections never authorize graph content operations. */
export function isUnmanagedNode(graphPath: string, nodeId: string): boolean {
  return (
    useResourceStore.getState().getGraphNode(graphPath, nodeId)?.capabilities?.managed === false
  );
}
