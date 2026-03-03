import { useExecutionStore } from '@/features/core/execution';

/**
 * Node Execution Hook
 *
 * 按 graphId 粒度化选择器：只提取本节点在指定图中的执行状态
 */
export function useNodeExecution(nodeId: string, graphId?: string) {
  const isExecuting = useExecutionStore((state) => {
    if (!graphId) return false;
    return state.graphs[graphId]?.nodeStates.get(nodeId)?.status === "executing";
  });
  const isCompleted = useExecutionStore((state) => {
    if (!graphId) return false;
    return state.graphs[graphId]?.executedNodes.has(nodeId) ?? false;
  });
  const nodeState = useExecutionStore((state) => {
    if (!graphId) return null;
    return state.graphs[graphId]?.nodeStates.get(nodeId) ?? null;
  });
  const hasError = nodeState?.status === "error";

  return {
    nodeState,
    isExecuting,
    isCompleted,
    hasError,
  };
}
