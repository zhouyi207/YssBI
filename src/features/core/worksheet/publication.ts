import type { WorksheetReadSnapshot } from './read';

export interface WorksheetPublicationCapability {
  readonly publishWorksheet: (snapshot: WorksheetReadSnapshot) => void;
  readonly beginPendingSave: (path: string, operationId: string) => void;
  readonly settlePendingSave: (path: string, operationId: string) => void;
}
