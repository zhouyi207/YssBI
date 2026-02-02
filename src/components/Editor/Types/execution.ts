/**
 * 执行状态类型定义
 */

export type ExecutionStatus = "idle" | "running" | "completed" | "error";

export interface NodeExecutionState {
  nodeId: string;
  status: "pending" | "executing" | "completed" | "error";
  timestamp: number;
}

export interface ExecutionState {
  status: ExecutionStatus;
  currentNodeId: string | null;
  executedNodes: Set<string>;
  nodeStates: Map<string, NodeExecutionState>;
  activeConnections: Set<string>; // 正在流动的连接线 (格式: "fromPinId->toPinId")
  completedConnections: Set<string>; // 已完成的连接线，显示数据流动画 (格式: "fromPinId->toPinId")
}

export interface ExecutionEvent {
  type: "node_start" | "node_complete" | "node_error" | "execution_start" | "execution_complete" | "connection_active";
  nodeId?: string;
  fromPinId?: string;
  toPinId?: string;
  error?: string;
  timestamp: number;
}
