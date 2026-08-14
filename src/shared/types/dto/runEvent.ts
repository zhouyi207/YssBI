import type { GraphOutputRefDto } from './executionDemand';
import type { ResultStateKind } from './result';

export type RunErrorCode =
  | 'invalidPlan'
  | 'cancelled'
  | 'activationIdExhausted'
  | 'deadlineExceeded'
  | 'kernelNotFound'
  | 'kernelFailed'
  | 'relationalBackendNotFound'
  | 'relationalOperatorInvalid'
  | 'relationalColumnMissing'
  | 'relationalTypeMismatch'
  | 'relationalInputShapeInvalid'
  | 'relationalHintInvalid'
  | 'missingRelationalFragment'
  | 'bridgeFailed'
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
  deadlineExceeded: true,
  kernelNotFound: true,
  kernelFailed: true,
  relationalBackendNotFound: true,
  relationalOperatorInvalid: true,
  relationalColumnMissing: true,
  relationalTypeMismatch: true,
  relationalInputShapeInvalid: true,
  relationalHintInvalid: true,
  missingRelationalFragment: true,
  bridgeFailed: true,
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

export type ResourceVersionSetDto = Record<string, string>;

export interface CompilationBasisDto {
  graphRevision: string;
  registryFingerprint: string;
  resourceVersions: ResourceVersionSetDto;
}

export interface RunCorrelationDto {
  projectSessionId: string;
  graphPath: string;
  graphRevision: string;
  registryFingerprint: string;
  resourceVersions: ResourceVersionSetDto;
  compileId: string;
  selectionDigest: string | null;
  runId: string | null;
  nodeId: string | null;
  nodeTypeId: string | null;
  parentCall: string | null;
}

export type RunEventKind =
  | { type: 'runStarted' }
  | { type: 'runCompleted' }
  | { type: 'runErrored'; code: 'deadlineExceeded'; phase: RunPhase }
  | { type: 'runErrored'; code: Exclude<RunErrorCode, 'deadlineExceeded'>; phase: null }
  | { type: 'runCancelled' }
  | { type: 'operationStarted'; operationIndex: number; activationId: string; attemptId: string }
  | { type: 'operationCompleted'; operationIndex: number; activationId: string; attemptId: string }
  | {
      type: 'operationErrored';
      operationIndex: number;
      activationId: string;
      attemptId: string;
      code: 'deadlineExceeded';
      phase: RunPhase;
    }
  | {
      type: 'operationErrored';
      operationIndex: number;
      activationId: string;
      attemptId: string;
      code: Exclude<RunErrorCode, 'deadlineExceeded'>;
      phase: null;
    }
  | {
      type: 'resultGroupChanged';
      activationId: string;
      resultIds: string[];
      state: ResultStateKind;
    }
  | {
      type: 'outputResultChanged';
      output: GraphOutputRefDto;
      generation: number | null;
      resultId: string;
    }
  | { type: 'openResultWindow'; resultId: string };

export const RUN_EVENT_KIND_TYPES = {
  runStarted: true,
  runCompleted: true,
  runErrored: true,
  runCancelled: true,
  operationStarted: true,
  operationCompleted: true,
  operationErrored: true,
  resultGroupChanged: true,
  outputResultChanged: true,
  openResultWindow: true,
} as const satisfies Record<RunEventKind['type'], true>;

export interface RunEvent {
  correlation: RunCorrelationDto;
  basis: CompilationBasisDto;
  kind: RunEventKind;
}

export interface ExecuteGraphResultDto {
  runId: string;
}
