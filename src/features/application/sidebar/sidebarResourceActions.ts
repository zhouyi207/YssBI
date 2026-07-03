import { i18n } from '@/app/i18n';
import { uiStore } from '@/features/core/ui/UIStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { ProjectService, type RevealProjectResourceRequest } from '@/services/project/projectService';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';

export async function revealProjectResourceInExplorer(
  request: RevealProjectResourceRequest,
): Promise<void> {
  try {
    await ProjectService.revealProjectResource(request);
  } catch (error) {
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
    store.upsertDocument(await WorksheetService.loadWorksheet(id));
  }
  store.updateDocument(id, { name: nextName });
  await store.saveDocument(id);
}
