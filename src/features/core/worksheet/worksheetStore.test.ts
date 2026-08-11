import { beforeEach, describe, expect, it, vi } from 'vitest';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { useHistoryStore } from '@/features/core/history';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { ResourceMutationCommittedHandler } from '@/features/core/sync/handlers/ProjectMutationEventHandler';
import type { WorksheetDocument } from '@/shared/types/domain/worksheet';
import { useWorksheetStore } from './worksheetStore';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import {
  isResourceDocumentDirty,
  markResourceDirty,
  resourceKey,
  useDocumentStateStore,
  useResourceStore,
} from '@/features/core/resource';

const projectInstanceId = '00000000-0000-0000-0000-000000000601';
const worksheetPath = 'worksheets/Report.yssbi-worksheet';

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => { resolve = settle; });
  return { promise, resolve };
}

function worksheet(revision: number, chartType: WorksheetDocument['chartType']): WorksheetDocument {
  return {
    schemaVersion: 3,
    revision,
    databaseId: 'database-1',
    chartType,
    encodings: { x: 'x', y: 'y' },
  };
}

function registerWorksheetResource(): void {
  useResourceStore.getState().upsertResource({
    id: worksheetPath,
    kind: 'worksheet',
    name: 'Report',
    uri: `yssbi://worksheet/${worksheetPath}`,
    exists: true,
    loaded: true,
    hasDirtyDocument: false,
    hasStaleDocument: false,
    hasConflictDocument: false,
  });
}

function worksheetResult(
  operationId: string,
  before: WorksheetDocument,
  after: WorksheetDocument,
) {
  return {
    operationId,
    projectInstanceId,
    publicationRevision: 1,
    moves: [],
    deltas: [{
      resource: { kind: 'worksheet' as const, key: worksheetPath },
      fromRevision: before.revision,
      toRevision: after.revision,
      causedBy: operationId,
      payload: {
        kind: 'worksheet' as const,
        patch: {
          before: {
            databaseId: before.databaseId,
            chartType: before.chartType,
            encodings: before.encodings,
          },
          after: {
            databaseId: after.databaseId,
            chartType: after.chartType,
            encodings: after.encodings,
          },
        },
      },
    }],
    projectionReplacements: [],
    projectionStatus: { status: 'complete' as const, expectedGraphPaths: [] },
    history: { canUndo: true, canRedo: false },
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

  it('keys documents explicitly without synthesizing index rows', () => {
    const document = worksheet(3, 'scatter');

    useWorksheetStore.getState().upsertDocument(worksheetPath, document);

    expect(useWorksheetStore.getState().documents).toEqual({ [worksheetPath]: document });
    expect(useWorksheetStore.getState().index).toEqual([]);
  });

  it('ignores a delayed save completion from a replaced project', async () => {
    const draft = worksheet(3, 'scatter');
    useWorksheetStore.getState().upsertDocument(worksheetPath, draft);
    markResourceDirty({ id: worksheetPath, kind: 'worksheet' }, true);
    const request = deferred<Awaited<ReturnType<typeof WorksheetService.saveWorksheet>>>();
    vi.spyOn(WorksheetService, 'saveWorksheet').mockReturnValue(request.promise);

    const completion = useWorksheetStore.getState().saveDocument(worksheetPath);
    await vi.waitFor(() => expect(WorksheetService.saveWorksheet).toHaveBeenCalled());
    useProjectIOStore.setState({ projectInstanceId: 'project-b' });
    projectPublicationCoordinator.startProject('project-b', 0);
    useWorksheetStore.getState().clear();
    request.resolve(worksheetResult(
      '00000000-0000-0000-0000-000000000502',
      draft,
      worksheet(4, 'line'),
    ));

    await expect(completion).resolves.toBe(false);
    expect(useWorksheetStore.getState().documents).toEqual({});
    expect(projectPublicationCoordinator.getSnapshotForTests()).toMatchObject({
      projectInstanceId: 'project-b',
      appliedRevision: 0,
    });
  });

  it('preserves a newer dirty edit while applying the save publication revision', async () => {
    const draft = worksheet(3, 'scatter');
    const saved = worksheet(4, 'scatter');
    registerWorksheetResource();
    useWorksheetStore.getState().upsertDocument(worksheetPath, draft);
    markResourceDirty({ id: worksheetPath, kind: 'worksheet' }, true);
    const request = deferred<Awaited<ReturnType<typeof WorksheetService.saveWorksheet>>>();
    vi.spyOn(WorksheetService, 'saveWorksheet').mockReturnValue(request.promise);

    const completion = useWorksheetStore.getState().saveDocument(worksheetPath);
    await vi.waitFor(() => expect(WorksheetService.saveWorksheet).toHaveBeenCalled());
    useWorksheetStore.getState().updateDocument(worksheetPath, { chartType: 'line' });
    request.resolve(worksheetResult(
      '00000000-0000-0000-0000-000000000503',
      draft,
      saved,
    ));

    await expect(completion).resolves.toBe(false);
    expect(useWorksheetStore.getState().documents[worksheetPath]).toMatchObject({
      chartType: 'line',
      revision: 4,
    });
    const key = resourceKey({ id: worksheetPath, kind: 'worksheet' });
    expect(isResourceDocumentDirty({ id: worksheetPath, kind: 'worksheet' })).toBe(true);
    expect(useDocumentStateStore.getState().documents[key]?.dirty).toBe(true);
    expect(useResourceStore.getState().resources[key]?.hasDirtyDocument).toBe(true);
    expect(projectPublicationCoordinator.getSnapshotForTests().appliedRevision).toBe(1);
  });

  it('clears dirty when an event-first save observes the submitted after state', async () => {
    const before = {
      ...worksheet(3, 'histogram'),
      encodings: { x: 'x', y: 'standard-premium' },
    };
    const submitted = {
      ...before,
      encodings: { x: 'x', y: 'signed-premium' },
    };
    const authoritative = { ...submitted, revision: 4 };
    registerWorksheetResource();
    useWorksheetStore.getState().upsertDocument(worksheetPath, submitted);
    markResourceDirty({ id: worksheetPath, kind: 'worksheet' }, true);
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit');
    vi.spyOn(WorksheetService, 'saveWorksheet').mockImplementation(
      async (_projectInstanceId, operationId) => {
        const result = worksheetResult(operationId, before, authoritative);
        new ResourceMutationCommittedHandler().handle({ result });
        await vi.waitFor(() => {
          expect(projectPublicationCoordinator.getSnapshotForTests().appliedRevision).toBe(1);
        });
        expect(isResourceDocumentDirty({ id: worksheetPath, kind: 'worksheet' })).toBe(false);
        return result;
      },
    );

    await expect(useWorksheetStore.getState().saveDocument(worksheetPath)).resolves.toBe(true);

    expect(useWorksheetStore.getState().documents[worksheetPath]).toEqual(authoritative);
    expect(isResourceDocumentDirty({ id: worksheetPath, kind: 'worksheet' })).toBe(false);
    expect(submit).toHaveBeenCalledTimes(2);
    expect(projectPublicationCoordinator.getSnapshotForTests().appliedRevision).toBe(1);
  });

  it('clears both dirty projections after a matching authoritative save', async () => {
    const draft = worksheet(3, 'scatter');
    const authoritative = worksheet(4, 'line');
    registerWorksheetResource();
    useWorksheetStore.getState().upsertDocument(worksheetPath, draft);
    markResourceDirty({ id: worksheetPath, kind: 'worksheet' }, true);
    vi.spyOn(WorksheetService, 'saveWorksheet').mockImplementation(
      async (_projectInstanceId, operationId) => worksheetResult(
        operationId,
        draft,
        authoritative,
      ),
    );

    await expect(useWorksheetStore.getState().saveDocument(worksheetPath)).resolves.toBe(true);

    expect(WorksheetService.saveWorksheet).toHaveBeenCalledWith(
      projectInstanceId,
      expect.any(String),
      worksheetPath,
      3,
      draft,
    );
    const key = resourceKey({ id: worksheetPath, kind: 'worksheet' });
    expect(useWorksheetStore.getState().documents[worksheetPath]).toEqual(authoritative);
    expect(useDocumentStateStore.getState().documents[key]?.dirty).toBe(false);
    expect(useResourceStore.getState().resources[key]?.hasDirtyDocument).toBe(false);
    expect(useHistoryStore.getState()).toMatchObject({ canUndo: true, canRedo: false });
  });
});
