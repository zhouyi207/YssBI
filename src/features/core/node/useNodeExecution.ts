import { useExecutionStore } from '@/features/core/execution';

/**
 * Node Execution Hook
 * 
 * 粒度化选择器：只提取本节点的执行状态，
 * 其他节点状态变化不会触发本组件 re-render
 */
export function useNodeExecution(nodeId: string) {
  const isExecuting = useExecutionStore((state) => state.currentNodeId === nodeId);
  const isCompleted = useExecutionStore((state) => state.executedNodes.has(nodeId));
  const nodeState = useExecutionStore((state) => state.nodeStates.get(nodeId) ?? null);
  const hasError = nodeState?.status === "error";

  return {
    nodeState,
    isExecuting,
    isCompleted,
    hasError,
  };
}
