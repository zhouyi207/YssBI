/**
 * UI Types - Execution
 * 
 * 执行状态相关的 UI 类型
 * 这些类型用于跟踪和显示图的执行状态
 */

/**
 * 执行状态
 */
export type ExecutionStatus = "idle" | "running" | "completed" | "error";

/**
 * 节点执行状态
 */
export interface NodeExecutionState {
    nodeId: string;
    status: "pending" | "executing" | "completed" | "error";
    timestamp: number;
}

/**
 * 执行状态
 * 用于跟踪整个图的执行状态
 */
export interface ExecutionState {
    status: ExecutionStatus;                    // 整体执行状态
    currentNodeId: string | null;               // 当前执行的节点 ID
    executedNodes: Set<string>;                 // 已执行的节点集合
    nodeStates: Map<string, NodeExecutionState>; // 节点状态映射
    activeConnections: Set<string>;             // 正在流动的连接线（格式: "fromPinId->toPinId"）
    completedConnections: Set<string>;          // 已完成的连接线（格式: "fromPinId->toPinId"）
}

/**
 * 执行事件
 * 用于通知执行过程中的各种事件
 */
export interface ExecutionEvent {
    type: "node_start" | "node_complete" | "node_error" | "execution_start" | "execution_complete" | "connection_active";
    nodeId?: string;
    fromPinId?: string;
    toPinId?: string;
    error?: string;
    timestamp: number;
}
