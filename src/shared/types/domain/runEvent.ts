import type { GraphOutputRefDto } from "@/shared/types/domain/executionDemand";

export const RUN_ERROR_CODES = {
  deadlineExceeded: true,
  kernelNotFound: true,
  kernelFailed: true,
  invalidNumericInput: true,
  shapeMismatch: true,
  invalidParameter: true,
  unalignedSeries: true,
  budgetExceeded: true,
  inputLayoutMismatch: true,
  outputContractMismatch: true,
  scientificFailure: true,
  divisionByZero: true,
  nonFiniteResult: true,
  resourceUnavailable: true,
  finalizationFailed: true,
} as const;

export type RunErrorCode = keyof typeof RUN_ERROR_CODES;

export const RUN_PHASES = {
  admission: true,
  planValidation: true,
  resourcePreparation: true,
  execution: true,
  finalization: true,
} as const;

export type RunPhase = keyof typeof RUN_PHASES;

export interface RunErrorOutcome {
  code: RunErrorCode;
  phase: RunPhase;
  source: ResultInspectionSource | null;
}

export interface GraphRunIdentityDto {
  executionSessionId: string;
  graphPath: string;
  runId: string;
}

export type RunEventKind =
  | { type: "runStarted"; outputs: GraphOutputRefDto[] }
  | { type: "runCompleted" }
  | ({ type: "runErrored" } & RunErrorOutcome)
  | { type: "runCancelled" }
  | {
      type: "pinPreviewResultReady";
      output: GraphOutputRefDto;
      generation: number;
      resultId: string;
    }
  | {
      type: "resultInspectionRequested";
      resultId: string;
      source: ResultInspectionSource;
    };

export interface ResultInspectionSource {
  graphPath: string;
  nodeId: string | null;
  portAddress: string | null;
}

export const RUN_EVENT_KIND_TYPES = {
  runStarted: true,
  runCompleted: true,
  runErrored: true,
  runCancelled: true,
  pinPreviewResultReady: true,
  resultInspectionRequested: true,
} as const satisfies Record<RunEventKind["type"], true>;

export interface RunEvent {
  run: GraphRunIdentityDto;
  kind: RunEventKind;
}
