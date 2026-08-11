import { create } from 'zustand';
import type { WorksheetDocument, WorksheetIndexEntry } from '@/shared/types/domain/worksheet';
import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { captureProjectCommandContext } from '@/features/application/projectCommandContext';

import {
  clearResourceDocumentState,
  isResourceDocumentDirty,
  markResourceDirty,
  markResourceLoaded,
} from '@/features/core/resource';

interface WorksheetStore {
  index: WorksheetIndexEntry[];
  documents: Record<string, WorksheetDocument>;
  setIndex(entries: WorksheetIndexEntry[]): void;
  upsertDocument(worksheetPath: string, document: WorksheetDocument): void;
  removeDocument(worksheetPath: string): void;
  clear(): void;
  updateDocument(
    worksheetPath: string,
    patch: Partial<WorksheetDocument>,
  ): WorksheetDocument | null;
  markDirty(worksheetPath: string): void;
  saveDocument(worksheetPath: string): Promise<boolean>;
}

export const useWorksheetStore = create<WorksheetStore>((set, get) => ({
  index: [],
  documents: {},

  setIndex: (entries) => set({ index: entries }),

  upsertDocument: (worksheetPath, document) =>
    set((state) => {
      markResourceLoaded({ id: worksheetPath, kind: 'worksheet' });
      return { documents: { ...state.documents, [worksheetPath]: document } };
    }),

  removeDocument: (worksheetPath) =>
    set((state) => {
      clearResourceDocumentState({ id: worksheetPath, kind: 'worksheet' });
      const documents = { ...state.documents };
      delete documents[worksheetPath];
      return {
        index: state.index.filter((entry) => entry.worksheetPath !== worksheetPath),
        documents,
      };
    }),

  clear: () => set({ index: [], documents: {} }),

  updateDocument: (worksheetPath, patch) => {
    const current = get().documents[worksheetPath];
    if (!current) return null;
    const next: WorksheetDocument = {
      ...current,
      ...patch,
      encodings: { ...current.encodings, ...patch.encodings },
    };
    get().upsertDocument(worksheetPath, next);
    get().markDirty(worksheetPath);
    return next;
  },

  markDirty: (worksheetPath) => {
    markResourceDirty({ id: worksheetPath, kind: 'worksheet' }, true);
  },

  saveDocument: async (worksheetPath) => {
    const document = get().documents[worksheetPath];
    if (!document) return false;
    const context = captureProjectCommandContext();
    const result = await WorksheetService.saveWorksheet(
      context.projectInstanceId,
      context.operationId,
      worksheetPath,
      document.revision,
      document,
    );
    if (!context.isCurrent()) return false;

    const expected = savedDocumentFromResult(
      result,
      worksheetPath,
      context.operationId,
      document,
    );
    await projectPublicationCoordinator.submit({ result });
    if (!context.isCurrent() || !expected) return false;
    const settled = get().documents[worksheetPath];
    return settled !== undefined
      && sameWorksheetDocument(settled, expected)
      && !isResourceDocumentDirty({ id: worksheetPath, kind: 'worksheet' });
  },
}));

function savedDocumentFromResult(
  result: ResourceMutationResultDto,
  worksheetPath: string,
  operationId: string,
  before: WorksheetDocument,
): WorksheetDocument | null {
  const delta = result.deltas.find((candidate) =>
    candidate.resource.kind === 'worksheet'
      && candidate.resource.key === worksheetPath
      && candidate.causedBy === operationId
      && candidate.fromRevision === before.revision
      && candidate.payload.kind === 'worksheet');
  if (!delta || delta.payload.kind !== 'worksheet') return null;
  return {
    ...before,
    ...delta.payload.patch.after,
    encodings: { ...delta.payload.patch.after.encodings },
    revision: delta.toRevision,
  };
}

function sameWorksheetDocument(left: WorksheetDocument, right: WorksheetDocument): boolean {
  return left.schemaVersion === right.schemaVersion
    && left.revision === right.revision
    && left.databaseId === right.databaseId
    && left.chartType === right.chartType
    && left.encodings.x === right.encodings.x
    && left.encodings.y === right.encodings.y;
}
