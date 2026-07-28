import type { HistoryStatusDto, ResourceKeyDto } from './editorMutation';

/** Project registry command DTOs. Kept at the IPC boundary to avoid service-local copies. */
export interface ProjectRecordRow {
  id: string;
  name: string;
  path: string;
  createdAt: string;
  lastOpenedAt: string | null;
  isFavorite: boolean;
}

export interface ScanProjectsResult {
  discovered: number;
  newlyRegistered: number;
  projects: ProjectRecordRow[];
}

export interface CleanupInvalidProjectsResult {
  removed: number;
}

export interface ProjectPathValidation {
  ok: boolean;
  message: string | null;
}

export interface ProjectSaveResultDto {
  projectInstanceId: string;
  operationId: string;
  publicationRevision: number;
  affectedResources: ResourceKeyDto[];
  indexInvalidated: boolean;
  history: HistoryStatusDto;
}
