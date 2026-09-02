import type { NodeData } from "@/features/domain/editorProjection/graphRuntimeTypes";

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

/** Find an internal node in a graph by nodeType (store-native, no links rebuild). */
export function findInternalNodeInGraph(graphPath: string, nodeType: string): NodeData | undefined {
  const state = useGraphProjectionStore.getState();
  const nodeIds = state.getGraphNodeIds(graphPath);
  for (const nodeId of nodeIds) {
    const node = state.getGraphNode(graphPath, nodeId);
    if (node?.nodeType === nodeType && node.isInternal) {
      return node;
    }
  }
  return undefined;
}
