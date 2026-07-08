/**
 * UI Types - Execution
 *
 * Tauri Channel 传输的执行事件 + 前端执行状态（按图独立存储）
 */

import type { Presentation, SourceDescriptor } from '@/features/core/resultSource';

// ─── Channel 事件类型（与后端 ExecutionEvent 枚举对应）───

export type ExecutionEvent =
  | { event: 'executionStart' }
  | { event: 'executionComplete'; data: { hasError: boolean } }
  | { event: 'nodeStart'; data: { nodeId: string } }
  | { event: 'nodeComplete'; data: { nodeId: string; durationMs?: number } }
  | { event: 'nodeError'; data: { nodeId: string; error: string; durationMs?: number } }
  | { event: 'connectionActive'; data: { fromPinId: string; toPinId: string } }
  | { event: 'connectionFlow'; data: { fromPinId: string; toPinId: string } }
  | {
      event: 'openSourceWindow';
      data: { sourceId: string; presentation: Presentation; windowTitle: string };
    }
  | {
      event: 'pinResultReady';
      data: {
        graphPath: string;
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
  status: "completed" | "error";
  timestamp: number;
  /** 后端计算耗时（毫秒），用于性能分析 */
  durationMs?: number;
}

export interface PinResultState {
  graphPath: string;
  nodeId: string;
  pinId: string;
  sourceId: string;
  descriptor: SourceDescriptor;
}

/** 单张图的执行状态 */
export interface GraphExecutionState {
  status: ExecutionStatus;
  nodeStates: Map<string, NodeExecutionState>;
  /** data 取数阶段已声明的 input 连线（ConnectionActive） */
  completedConnections: Set<string>;
  /** data 值已就绪、沿 output→input 流动的连线（ConnectionFlow）；exec 仍只用 completedConnections */
  flowingConnections: Set<string>;
  recording: RecordedEvent[];
  graphDirty: boolean;
  /** output pin id -> latest backend source descriptor */
  pinResults: Map<string, PinResultState>;
}

/** 全局执行状态 */
export interface ExecutionState {
  /** 按 graphPath 存储的执行状态 */
  graphs: Record<string, GraphExecutionState>;
  /** 当前正在回放的 graphPath */
  playbackGraphPath: string | null;
  isPlaying: boolean;
}
