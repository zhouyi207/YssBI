import { beforeEach, describe, expect, it, vi } from 'vitest';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { useHistoryStore } from '@/features/core/history';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import type { WorksheetDocument } from '@/shared/types/domain/worksheet';
import { useWorksheetStore } from './worksheetStore';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import {
  isResourceDocumentDirty,
  markResourceDirty,
  useDocumentStateStore,
  useResourceStore,
} from '@/features/core/resource';

const projectInstanceId = '00000000-0000-0000-0000-000000000601';

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => { resolve = settle; });
  return { promise, resolve };
}

function worksheet(revision: number, name: string): WorksheetDocument {
  return {
    schemaVersion: 3,
    revision,
    id: 'worksheet-1',
    name,
    databaseId: 'database-1',
    chartType: 'scatter',
    encodings: { x: 'x', y: 'y' },
  };
}

describe('worksheet authoritative mutation results', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    useWorksheetStore.getState().clear();
    useDocumentStateStore.getState().clear();
    useResourceStore.getState().clear();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useProjectIOStore.setState({ projectInstanceId });
    useHistoryStore.setState({ canUndo: false, canRedo: false, pending: false });
  });

  it('ignores a delayed save completion from a replaced project', async () => {
    const draft = worksheet(3, 'Draft');
    useWorksheetStore.getState().upsertDocument(draft);
    markResourceDirty({ id: draft.id, kind: 'worksheet' }, true);
    const request = deferred<Awaited<ReturnType<typeof WorksheetService.saveWorksheet>>>();
    vi.spyOn(WorksheetService, 'saveWorksheet').mockReturnValue(request.promise);

    const completion = useWorksheetStore.getState().saveDocument(draft.id);
    await vi.waitFor(() => expect(WorksheetService.saveWorksheet).toHaveBeenCalled());
    useProjectIOStore.setState({ projectInstanceId: 'project-b' });
    projectPublicationCoordinator.startProject('project-b', 0);
    useWorksheetStore.getState().clear();
    request.resolve({
      operationId: '00000000-0000-0000-0000-000000000502',
      document: worksheet(4, 'Saved in project A'),
      result: {
        operationId: '00000000-0000-0000-0000-000000000502',
        projectInstanceId,
        publicationRevision: 1,
        moves: [],
        deltas: [],
        worksheetDeltas: [{ id: draft.id, before: draft, after: worksheet(4, 'Saved in project A') }],
        projectionReplacements: [],
        projectionStatus: { status: 'complete', expectedGraphPaths: [] },
        history: { canUndo: true, canRedo: false },
      },
    });

    await expect(completion).resolves.toBe(false);
    expect(useWorksheetStore.getState().documents).toEqual({});
    expect(projectPublicationCoordinator.getSnapshotForTests()).toMatchObject({
      projectInstanceId: 'project-b',
      appliedRevision: 0,
    });
  });

  it('preserves a newer dirty edit while applying the save publication revision', async () => {
    const draft = worksheet(3, 'Draft');
    const saved = worksheet(4, 'Draft');
    useWorksheetStore.getState().upsertDocument(draft);
    markResourceDirty({ id: draft.id, kind: 'worksheet' }, true);
    const request = deferred<Awaited<ReturnType<typeof WorksheetService.saveWorksheet>>>();
    vi.spyOn(WorksheetService, 'saveWorksheet').mockReturnValue(request.promise);

    const completion = useWorksheetStore.getState().saveDocument(draft.id);
    await vi.waitFor(() => expect(WorksheetService.saveWorksheet).toHaveBeenCalled());
    useWorksheetStore.getState().updateDocument(draft.id, { name: 'Edited while saving' });
    request.resolve({
      operationId: '00000000-0000-0000-0000-000000000503',
      document: saved,
      result: {
        operationId: '00000000-0000-0000-0000-000000000503',
        projectInstanceId,
        publicationRevision: 1,
        moves: [],
        deltas: [],
        worksheetDeltas: [{ id: draft.id, before: draft, after: saved }],
        projectionReplacements: [],
        projectionStatus: { status: 'complete', expectedGraphPaths: [] },
        history: { canUndo: true, canRedo: false },
      },
    });

    await expect(completion).resolves.toBe(false);
    expect(useWorksheetStore.getState().documents[draft.id]).toMatchObject({
      name: 'Edited while saving',
      revision: 4,
    });
    expect(isResourceDocumentDirty({ id: draft.id, kind: 'worksheet' })).toBe(true);
    expect(projectPublicationCoordinator.getSnapshotForTests().appliedRevision).toBe(1);
  });

  it('installs the authoritative document returned by save', async () => {
    const draft = worksheet(3, 'Draft');
    const authoritative = worksheet(4, 'Canonical');
    useWorksheetStore.getState().upsertDocument(draft);
    vi.spyOn(WorksheetService, 'saveWorksheet').mockResolvedValue({
      operationId: '00000000-0000-0000-0000-000000000501',
      document: authoritative,
      result: {
        operationId: '00000000-0000-0000-0000-000000000501',
        projectInstanceId,
        publicationRevision: 1,
        moves: [],
        deltas: [],
        worksheetDeltas: [{ id: draft.id, before: draft, after: authoritative }],
        projectionReplacements: [],
        projectionStatus: { status: 'complete', expectedGraphPaths: [] },
        history: { canUndo: true, canRedo: false },
      },
    });

    await expect(useWorksheetStore.getState().saveDocument(draft.id)).resolves.toBe(true);

    expect(WorksheetService.saveWorksheet).toHaveBeenCalledWith(
      projectInstanceId,
      expect.any(String),
      draft,
    );
    expect(useWorksheetStore.getState().documents[draft.id]).toEqual(authoritative);
    expect(useHistoryStore.getState()).toMatchObject({ canUndo: true, canRedo: false });
  });
});
