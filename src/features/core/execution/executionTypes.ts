import type { RunPhase, ResultInspectionSource } from "@/shared/types/domain/runEvent";

export type ExecutionStatus = "idle" | "submitting" | "running" | "completed" | "error" | "unknown";

export interface RunFailureProjection {
  runId: string | null;
  code: string;
  phase: RunPhase | null;
  source: ResultInspectionSource | null;
  incidentId: string | null;
}

/** 单张图的执行状态 */
export interface GraphExecutionState {
  status: ExecutionStatus;
  runId: string | null;
  /** Identity of the current local run request; edits revoke late callbacks. */
  request: object | null;
  runFailure: RunFailureProjection | null;
}

/** 全局执行状态 */
export interface ExecutionState {
  /** 按 graphPath 存储的执行状态 */
  graphs: Record<string, GraphExecutionState>;
}
