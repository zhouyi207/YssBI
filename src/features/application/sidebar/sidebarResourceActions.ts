import { i18n } from '@/app/i18n';
import { uiStore } from '@/features/core/ui/UIStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { ProjectService, type RevealProjectResourceRequest } from '@/services/project/projectService';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { captureProjectCommandContext } from '@/features/application/projectCommandContext';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';

export async function revealProjectResourceInExplorer(
  request: RevealProjectResourceRequest,
): Promise<void> {
  const context = captureProjectCommandContext();
  try {
    await ProjectService.revealProjectResource(context.projectInstanceId, request);
    if (!context.isCurrent()) return;
  } catch (error) {
    if (!context.isCurrent()) return;
    uiStore.showToast(
      i18n.t('contextMenu.sidebar.revealInExplorerFailed', {
        error: formatErrorMessage(error, 'Unknown error'),
      }),
      'error',
    );
  }
}

export async function renameWorksheetResource(id: string, nextName: string): Promise<void> {
  const store = useWorksheetStore.getState();
  if (!store.documents[id]) {
    const context = captureProjectCommandContext();
    const document = await WorksheetService.loadWorksheet(context.projectInstanceId, id);
    if (!context.isCurrent()) return;
    store.upsertDocument(document);
  }
  store.updateDocument(id, { name: nextName });
  await store.saveDocument(id);
}
