import type { GraphOutputRefDto } from '@/shared/types/domain/executionDemand';
import type { PortAddressDto } from '@/shared/types/domain/editorProjection';

export type RunErrorCode =
  | 'invalidPlan'
  | 'cancelled'
  | 'activationIdExhausted'
  | 'runtimeIdExhausted'
  | 'deadlineExceeded'
  | 'kernelNotFound'
  | 'kernelFailed'
  | 'relationalBackendNotFound'
  | 'relationalOperatorInvalid'
  | 'relationalColumnMissing'
  | 'relationalTypeMismatch'
  | 'relationalInputShapeInvalid'
  | 'relationalHintInvalid'
  | 'stream'
  | 'missingValue'
  | 'invalidCondition'
  | 'outputCount'
  | 'operationAlreadyExecuted'
  | 'unsatisfiedEffectDependency'
  | 'loopLimitExceeded'
  | 'functionPlanNotFound'
  | 'functionPlanFailed'
  | 'recursionLimitExceeded'
  | 'projectDraining'
  | 'resourceSnapshotMismatch'
  | 'resourceAcquire';

export const RUN_ERROR_CODES = {
  invalidPlan: true,
  cancelled: true,
  activationIdExhausted: true,
  runtimeIdExhausted: true,
  deadlineExceeded: true,
  kernelNotFound: true,
  kernelFailed: true,
  relationalBackendNotFound: true,
  relationalOperatorInvalid: true,
  relationalColumnMissing: true,
  relationalTypeMismatch: true,
  relationalInputShapeInvalid: true,
  relationalHintInvalid: true,
  stream: true,
  missingValue: true,
  invalidCondition: true,
  outputCount: true,
  operationAlreadyExecuted: true,
  unsatisfiedEffectDependency: true,
  loopLimitExceeded: true,
  functionPlanNotFound: true,
  functionPlanFailed: true,
  recursionLimitExceeded: true,
  projectDraining: true,
  resourceSnapshotMismatch: true,
  resourceAcquire: true,
} as const satisfies Record<RunErrorCode, true>;

export type RunPhase =
  | 'queueWait'
  | 'kernel'
  | 'streamSend'
  | 'streamReceive'
  | 'adapterIo'
  | 'resultPublication'
  | 'cleanup';

export const RUN_PHASES = {
  queueWait: true,
  kernel: true,
  streamSend: true,
  streamReceive: true,
  adapterIo: true,
  resultPublication: true,
  cleanup: true,
} as const satisfies Record<RunPhase, true>;

export type RunErrorOutcome =
  | { code: 'deadlineExceeded'; phase: RunPhase }
  | {
      code: Exclude<RunErrorCode, 'deadlineExceeded'>;
      phase: null;
    };

export interface GraphRunIdentityDto {
  projectSessionId: string;
  graphPath: string;
  runId: string;
}

export type RunEventKind =
  | { type: 'runStarted' }
  | { type: 'runCompleted' }
  | ({ type: 'runErrored' } & RunErrorOutcome)
  | { type: 'runCancelled' }
  | {
      type: 'pinPreviewResultReady';
      output: GraphOutputRefDto;
      generation: number;
      resultId: string;
    }
  | {
      type: 'resultInspectionRequested';
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
} as const satisfies Record<RunEventKind['type'], true>;

export interface RunEvent {
  run: GraphRunIdentityDto;
  kind: RunEventKind;
}

export type RunOutputStream = 'stdout' | 'stderr';

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

export type RunOutputStatus = 'truncated' | 'dropped';

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
