import { useGraphProjectionStore } from "./graphProjectionStore";

function projectedNodeCapabilities(graphPath: string, nodeId: string) {
  return useGraphProjectionStore.getState().getGraphNode(graphPath, nodeId)?.capabilities;
}

export function canCopyNode(graphPath: string, nodeId: string): boolean {
  const capabilities = projectedNodeCapabilities(graphPath, nodeId);
  return capabilities?.managed === false && capabilities.canCopy === true;
}

export function canDeleteNode(graphPath: string, nodeId: string): boolean {
  const capabilities = projectedNodeCapabilities(graphPath, nodeId);
  return capabilities?.managed === false && capabilities.canDelete === true;
}

export function canCutNode(graphPath: string, nodeId: string): boolean {
  return canCopyNode(graphPath, nodeId) && canDeleteNode(graphPath, nodeId);
}
