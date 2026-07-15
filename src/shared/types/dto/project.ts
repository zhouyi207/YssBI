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
