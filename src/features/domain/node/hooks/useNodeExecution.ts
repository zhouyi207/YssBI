import { useExecutionStore } from '@/features/core/execution';

/**
 * Node Execution Hook
 * 
 * 职责：
 * - 获取节点的执行状态
 * - 提供执行状态的判断逻辑
 * 
 * 使用场景：
 * - Node 组件需要显示执行状态
 * - 需要根据执行状态改变样式
 */
export function useNodeExecution(nodeId: string) {
  const currentNodeId = useExecutionStore((state) => state.currentNodeId);
  const nodeStates = useExecutionStore((state) => state.nodeStates);
  const executedNodes = useExecutionStore((state) => state.executedNodes);

  const nodeState = nodeStates.get(nodeId);
  const isExecuting = currentNodeId === nodeId;
  const isCompleted = executedNodes.has(nodeId);
  const hasError = nodeState?.status === "error";

  return {
    nodeState,
    isExecuting,
    isCompleted,
    hasError,
  };
}
