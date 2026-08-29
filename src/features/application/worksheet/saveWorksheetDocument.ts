import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { isResourceDocumentDirty } from '@/features/core/resource';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import type { ResourceMutationResultDto } from '@/shared/types/domain/editorMutation';
import type { WorksheetDocument } from '@/shared/types/domain/worksheet';
import { captureProjectCommandContext } from '@/features/application/projectCommandContext';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';

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

/** Saves the current worksheet draft through the Application mutation owner. */
export async function saveWorksheetDocument(worksheetPath: string): Promise<boolean> {
  const document = useWorksheetStore.getState().documents[worksheetPath];
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
  const settled = useWorksheetStore.getState().documents[worksheetPath];
  return settled !== undefined
    && sameWorksheetDocument(settled, expected)
    && !isResourceDocumentDirty({ id: worksheetPath, kind: 'worksheet' });
}
