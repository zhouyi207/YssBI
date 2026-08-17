import type { HistoryStatusDto, ResourceKeyDto } from './editorMutation';

/** Project registry command DTOs. Kept at the IPC boundary to avoid service-local copies. */
export interface ProjectRecordRow {
  id: string;
  name: string;
  path: string;
  createdAt: string;
  lastOpenedAt: string | null;
  isFavorite: boolean;
  rootIdentity: string;
}

export type LifecycleMutationKind =
  | 'saveAs'
  | 'create'
  | 'delete'
  | 'registryCleanup'
  | 'load'
  | 'clear';
export type LifecycleMutationPhase =
  | 'destinationCommitted'
  | 'tombstoneCommitted'
  | 'registryCommitted'
  | 'authorityCommitted';
export type LifecycleMutationOutcome =
  | 'committed'
  | 'registryFailed'
  | 'activationFailed'
  | 'registryPending'
  | 'cleanupPending';

export interface LifecycleRecoveryDto {
  required: boolean;
  action: string;
  path: string | null;
  identity: string | null;
}

export interface LifecycleMutationResultDto {
  operationId: string;
  kind: LifecycleMutationKind;
  oldProjectInstanceId: string | null;
  newProjectInstanceId: string | null;
  phase: LifecycleMutationPhase;
  outcome: LifecycleMutationOutcome;
  record: ProjectRecordRow | null;
  path: string | null;
  recovery: LifecycleRecoveryDto | null;
  invalidation: {
    project: boolean;
    registry: boolean;
  };
}

export interface ScanProjectsResult {
  discovered: number;
  newlyRegistered: number;
  projects: ProjectRecordRow[];
}

export interface CleanupInvalidProjectsResult {
  removed: number;
}


export interface ProjectSaveResultDto {
  projectInstanceId: string;
  operationId: string;
  publicationRevision: number;
  affectedResources: ResourceKeyDto[];
  indexInvalidated: boolean;
  history: HistoryStatusDto;
}
