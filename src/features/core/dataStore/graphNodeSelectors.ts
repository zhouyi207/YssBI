import type { NodeData } from '@/shared/types/store/graph';
import { isShellNodeDefinition } from '@/shared/types/domain';
import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import { useGraphDataStore } from './graphDataStore';

/** Whether a node instance is a system-managed shell (Event Begin, Function Entry/Return). */
export function isShellNode(graphPath: string, nodeId: string): boolean {
  const node = useGraphDataStore.getState().getGraphNode(graphPath, nodeId);
  if (!node) return false;
  const def = useNodeRegistryStore.getState().getDefinition(node.nodeType);
  return isShellNodeDefinition(def);
}

/** Find an internal node in a graph by nodeType (store-native, no links rebuild). */
export function findInternalNodeInGraph(
  graphPath: string,
  nodeType: string,
): NodeData | undefined {
  const state = useGraphDataStore.getState();
  const nodeIds = state.getGraphNodeIds(graphPath);
  for (const nodeId of nodeIds) {
    const node = state.getGraphNode(graphPath, nodeId);
    if (node?.nodeType === nodeType && node.isInternal) {
      return node;
    }
  }
  return undefined;
}
