import type {
  GraphRunIdentityDto,
  RunPhase,
  ResultInspectionSource,
} from "@/shared/types/domain/runEvent";
import type { GraphOutputRefDto } from "@/shared/types/domain/executionDemand";

export type ExecutionStatus = "idle" | "submitting" | "running" | "completed" | "error" | "unknown";

export interface RunFailureProjection {
  run: GraphRunIdentityDto;
  code: string;
  phase: RunPhase | null;
  source: ResultInspectionSource | null;
  incidentId: string | null;
}

/** 单张图的执行状态 */
export interface GraphExecutionState {
  status: ExecutionStatus;
  run: GraphRunIdentityDto | null;
  /** Identity of the current local run request; edits revoke late callbacks. */
  request: object | null;
  runFailure: RunFailureProjection | null;
  outputRuns: Record<string, OutputRun>;
}

export interface OutputRun {
  run: GraphRunIdentityDto;
  output: GraphOutputRefDto;
  active: boolean;
  resultRevision: string;
}

/** 全局执行状态 */
export interface ExecutionState {
  /** 按 graphPath 存储的执行状态 */
  graphs: Record<string, GraphExecutionState>;
}
