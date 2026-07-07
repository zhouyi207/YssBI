import type { NodeData, RuntimeNodeInput } from '@/shared/types/store/graph';
import { isShellNodeDefinition } from '@/shared/types/domain';
import { useNodeRegistryStore } from '@/features/core/nodeRegister';
import { useGraphDataStore } from './graphDataStore';
import { resolveNodeViewMeta } from './serialization';

function isPresent<T>(value: T | null | undefined): value is T {
  return value != null;
}

/** Whether a node instance is a system-managed shell (Event Begin, Function Entry/Return). */
export function isShellNode(graphId: string, nodeId: string): boolean {
  const node = useGraphDataStore.getState().getGraphNode(graphId, nodeId);
  if (!node) return false;
  const def = useNodeRegistryStore.getState().getDefinition(node.nodeType);
  return isShellNodeDefinition(def);
}

/** Find an internal node in a graph by nodeType (store-native, no links rebuild). */
export function findInternalNodeInGraph(
  graphId: string,
  nodeType: string,
): NodeData | undefined {
  const state = useGraphDataStore.getState();
  const nodeIds = state.getGraphNodeIds(graphId);
  for (const nodeId of nodeIds) {
    const node = state.getGraphNode(graphId, nodeId);
    if (node?.nodeType === nodeType && node.isInternal) {
      return node;
    }
  }
  return undefined;
}

/** Build runtime node inputs for replaceGraphNodes from normalized store state. */
export function buildRuntimeNodesFromStore(graphId: string): RuntimeNodeInput[] {
  const state = useGraphDataStore.getState();
  return state.getGraphNodeIds(graphId)
    .map((nodeId) => {
      const node = state.getGraphNode(graphId, nodeId);
      if (!node) return null;
      const meta = resolveNodeViewMeta(node);
      return {
        id: node.id,
        graphId: node.graphId,
        nodeType: meta.nodeType,
        category: meta.category,
        title: meta.title,
        position: node.position,
        inputs: node.inputs,
        outputs: node.outputs,
        uiStyle: meta.uiStyle,
        description: meta.description,
        isInternal: node.isInternal,
        paramsKind: node.paramsKind,
        variableId: node.variableId,
        variableName: node.variableName,
        variableType: node.variableType,
        subGraphId: node.subGraphId,
        dataframeId: node.dataframeId,
      } satisfies RuntimeNodeInput;
    })
    .filter(isPresent);
}
