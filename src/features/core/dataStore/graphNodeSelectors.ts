import { useGraphProjectionStore } from "./graphProjectionStore";

/** Missing projections never authorize graph content operations. */
export function isUnmanagedNode(graphPath: string, nodeId: string): boolean {
  return (
    useGraphProjectionStore.getState().getGraphNode(graphPath, nodeId)?.capabilities?.managed ===
    false
  );
}
