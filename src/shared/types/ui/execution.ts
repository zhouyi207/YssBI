/**
 * UI Types - Execution
 *
 * Tauri Channel 传输的执行事件 + 前端执行状态（按图独立存储）
 */

import type { Presentation, SourceDescriptor } from '@/features/core/dataView';

// ─── Channel 事件类型（与后端 ExecutionEvent 枚举对应）───

export type ExecutionEvent =
  | { event: 'executionStart' }
  | { event: 'executionComplete'; data: { hasError: boolean } }
  | { event: 'nodeStart'; data: { nodeId: string } }
  | { event: 'nodeComplete'; data: { nodeId: string; durationMs?: number } }
  | { event: 'nodeError'; data: { nodeId: string; error: string; durationMs?: number } }
  | { event: 'connectionActive'; data: { fromPinId: string; toPinId: string } }
  | {
      event: 'openSourceWindow';
      data: { sourceId: string; presentation: Presentation; windowTitle: string };
    }
  | {
      event: 'pinResultReady';
      data: {
        graphId: string;
        nodeId: string;
        pinId: string;
        sourceId: string;
        descriptor: SourceDescriptor;
      };
    };

/** 带时间戳的录制事件 */
export interface RecordedEvent {
  event: ExecutionEvent;
  timestamp: number;
}

// ─── 前端执行状态 ───

export type ExecutionStatus = "idle" | "running" | "completed" | "error";

export interface NodeExecutionState {
  nodeId: string;
  status: "pending" | "executing" | "completed" | "error";
  timestamp: number;
  /** 后端计算耗时（毫秒），用于性能分析 */
  durationMs?: number;
}

export interface PinResultState {
  graphId: string;
  nodeId: string;
  pinId: string;
  sourceId: string;
  descriptor: SourceDescriptor;
}

/** 单张图的执行状态 */
export interface GraphExecutionState {
  status: ExecutionStatus;
  currentNodeId: string | null;
  executedNodes: Set<string>;
  nodeStates: Map<string, NodeExecutionState>;
  completedConnections: Set<string>;
  errorConnections: Set<string>;
  recording: RecordedEvent[];
  graphDirty: boolean;
  /** 节点 ID -> 后端计算耗时(ms)，用于性能分析 */
  nodeDurations: Map<string, number>;
  /** output pin id -> latest backend source descriptor */
  pinResults: Map<string, PinResultState>;
}

/** 全局执行状态 */
export interface ExecutionState {
  /** 按 graphId 存储的执行状态 */
  graphs: Record<string, GraphExecutionState>;
  /** 当前正在回放的 graphId */
  playbackGraphId: string | null;
  isPlaying: boolean;
}
