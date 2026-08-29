import { WorksheetService } from '@/services/worksheet/worksheetService';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { captureProjectCommandContext } from '@/features/application/projectCommandContext';
import type { WorksheetDocument } from '@/shared/types/domain/worksheet';

/** Load a worksheet for a View and commit only if its captured project is current. */
export async function loadWorksheetDocumentForView(
  worksheetPath: string,
): Promise<WorksheetDocument | null> {
  const context = captureProjectCommandContext();
  try {
    const document = await WorksheetService.loadWorksheet(
      context.projectInstanceId,
      worksheetPath,
    );
    if (!context.isCurrent()) return null;
    useWorksheetStore.getState().upsertDocument(worksheetPath, document);
    return document;
  } catch {
    return null;
  }
}
