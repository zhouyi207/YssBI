import type { GraphOutputRefDto } from './executionDemand';

export type RunErrorCode =
  | 'invalidPlan'
  | 'cancelled'
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
  | { type: 'runErrored'; code: RunErrorCode }
  | { type: 'runCancelled' }
  | { type: 'operationStarted'; operationIndex: number; activationId: string }
  | { type: 'operationCompleted'; operationIndex: number; activationId: string }
  | {
      type: 'operationErrored';
      operationIndex: number;
      activationId: string;
      code: RunErrorCode;
    }
  | { type: 'valueReady'; valueIndex: number; sourceId: string }
  | { type: 'resultReady'; name: string; sourceId: string }
  | { type: 'outputReady'; output: GraphOutputRefDto; sourceId: string };

export const RUN_EVENT_KIND_TYPES = {
  runStarted: true,
  runCompleted: true,
  runErrored: true,
  runCancelled: true,
  operationStarted: true,
  operationCompleted: true,
  operationErrored: true,
  valueReady: true,
  resultReady: true,
  outputReady: true,
} as const satisfies Record<RunEventKind['type'], true>;

export interface RunEvent {
  correlation: RunCorrelationDto;
  basis: CompilationBasisDto;
  kind: RunEventKind;
}

export interface ExecuteGraphResultDto {
  runId: string;
}
