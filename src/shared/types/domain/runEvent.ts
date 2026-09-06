import type { GraphOutputRefDto } from "@/shared/types/domain/executionDemand";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";

export const RUN_ERROR_CODES = {
  deadlineExceeded: true,
  kernelNotFound: true,
  kernelFailed: true,
  invalidNumericInput: true,
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
  projectSessionId: string;
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

export type RunOutputStream = "stdout" | "stderr";

export const RUN_OUTPUT_STREAMS = {
  stdout: true,
  stderr: true,
} as const satisfies Record<RunOutputStream, true>;

export interface RunOutputEvent {
  runId: string;
  sequence: number;
  stream: RunOutputStream;
  text: string;
  sourceGraphPath: string;
  sourceNodeId: string;
  sourcePort: PortAddressDto;
}

export type RunOutputStatus = "truncated" | "dropped";

export const RUN_OUTPUT_STATUSES = {
  truncated: true,
  dropped: true,
} as const satisfies Record<RunOutputStatus, true>;

export interface RunOutputStatusEvent {
  runId: string;
  sequence: number;
  stream: RunOutputStream;
  status: RunOutputStatus;
  sourceGraphPath: string;
  sourceNodeId: string;
  sourcePort: PortAddressDto;
}

export type RunOutputChannelEvent = RunOutputEvent | RunOutputStatusEvent;
export type ExecutionChannelEvent = RunEvent | RunOutputChannelEvent;
