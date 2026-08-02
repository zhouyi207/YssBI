export type RunErrorCode =
  | 'invalidPlan'
  | 'cancelled'
  | 'kernelNotFound'
  | 'kernelFailed'
  | 'relationalBackendNotFound'
  | 'relationalAcquire'
  | 'relationalFailed'
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
  | { type: 'resultReady'; name: string; sourceId: string };

export interface RunEvent {
  correlation: RunCorrelationDto;
  basis: CompilationBasisDto;
  kind: RunEventKind;
}

export interface ExecuteGraphResultDto {
  runId: string;
}
