import { beforeEach, describe, expect, it, vi } from 'vitest';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { useHistoryStore } from '@/features/core/history';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import type { WorksheetDocument } from '@/shared/types/domain/worksheet';
import { useWorksheetStore } from './worksheetStore';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';

const projectInstanceId = '00000000-0000-0000-0000-000000000601';

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
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useProjectIOStore.setState({ projectInstanceId });
    useHistoryStore.setState({ canUndo: false, canRedo: false, pending: false });
  });

  it('installs the authoritative document returned by save', async () => {
    const draft = worksheet(3, 'Draft');
    const authoritative = worksheet(4, 'Canonical');
    useWorksheetStore.getState().upsertDocument(draft);
    vi.spyOn(WorksheetService, 'saveWorksheet').mockResolvedValue({
      operationId: '00000000-0000-0000-0000-000000000501',
      document: authoritative,
      result: {
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

    await useWorksheetStore.getState().saveDocument(draft.id);

    expect(WorksheetService.saveWorksheet).toHaveBeenCalledWith(
      projectInstanceId,
      expect.any(String),
      draft,
    );
    expect(useWorksheetStore.getState().documents[draft.id]).toEqual(authoritative);
    expect(useHistoryStore.getState()).toMatchObject({ canUndo: true, canRedo: false });
  });
});
