import { logger } from "@/utils/appLogger";
import { i18n } from '@/app/i18n';

import { ProjectService, type RevealProjectResourceRequest } from '@/services/project/projectService';

import { captureProjectCommandContext } from '@/features/application/projectCommandContext';
import { renameResource } from '@/features/application/resource/resourceActions';
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
    logger.notify.error(i18n.t('contextMenu.sidebar.revealInExplorerFailed', {
        error: formatErrorMessage(error, 'Unknown error'),
      }), "UI");
  }
}

export async function renameWorksheetResource(
  worksheetPath: string,
  nextName: string,
): Promise<void> {
  try {
    await renameResource({ id: worksheetPath, kind: 'worksheet' }, nextName);
  } catch (error) {
    throw new Error(formatErrorMessage(error, 'Worksheet rename failed'));
  }
}
