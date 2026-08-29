/**
 * UI Types - Execution
 *
 * Tauri Channel 传输的执行事件 + 前端执行状态（按图独立存储）
 */

import type { PortAddressDto } from '@/shared/types/domain/editorProjection';
import type { PinResultEntry } from '@/shared/types/domain/result';
import type { RunOutputChannelEvent } from '@/shared/types/domain/runEvent';

// ─── Channel 事件类型（与后端 ExecutionEvent 枚举对应）───

export type ExecutionEvent =
  | { event: 'executionStart' }
  | { event: 'executionComplete'; data: { hasError: boolean } }
  | { event: 'nodeStart'; data: { nodeId: string } }
  | { event: 'nodeComplete'; data: { nodeId: string; durationMs?: number } }
  | { event: 'nodeError'; data: { nodeId: string; error: string; durationMs?: number } }
  | { event: 'connectionActive'; data: { fromPinId: string; toPinId: string } }
  | { event: 'connectionFlow'; data: { fromPinId: string; toPinId: string } }

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


export interface PinHistoryProjection {
  graphPath: string;
  output: PortAddressDto;
  entries: PinResultEntry[];
  selectedResultId: string | null;
}

export interface RunOutputProjection {
  runId: string | null;
  entries: RunOutputChannelEvent[];
  projectionDropped: boolean;
}

export interface PinPreviewState {
  graphPath: string;
  port: PortAddressDto;
  generation: number;
  status: 'pending' | 'ready' | 'error';
  resultId: string | null;
  error: string | null;
}

/** 单张图的执行状态 */
export interface GraphExecutionState {
  status: ExecutionStatus;
  runId: string | null;
  nodeStates: Map<string, NodeExecutionState>;
  /** data 取数阶段已声明的 input 连线（ConnectionActive） */
  completedConnections: Set<string>;
  /** data 值已就绪、沿 output→input 流动的连线（ConnectionFlow）；exec 仍只用 completedConnections */
  flowingConnections: Set<string>;
  recording: RecordedEvent[];
  graphDirty: boolean;
  runOutput: RunOutputProjection;

  /** Backend output-Pin history projections keyed by exact graph path and address. */
  pinHistories: Map<string, PinHistoryProjection>;
  /** Stable `(graphPath, PortAddressDto)` preview projections. */
  pinPreviews: Map<string, PinPreviewState>;
}

/** 全局执行状态 */
export interface ExecutionState {
  /** 按 graphPath 存储的执行状态 */
  graphs: Record<string, GraphExecutionState>;
  /** 当前正在回放的 graphPath */
  playbackGraphPath: string | null;
  isPlaying: boolean;
}
