import { logger } from '@/utils/appLogger';

import { ProjectService, type RevealProjectResourceRequest } from '@/services/project/projectService';
import { normalizeIpcError } from '@/services/ipc';

import { captureProjectCommandContext } from '@/features/application/projectCommandContext';
import { renameResource } from '@/features/application/resource/resourceActions';


export async function revealProjectResourceInExplorer(
  request: RevealProjectResourceRequest,
): Promise<void> {
  const context = captureProjectCommandContext();
  try {
    await ProjectService.revealProjectResource(context.projectInstanceId, request);
    if (!context.isCurrent()) return;
  } catch (error) {
    if (!context.isCurrent()) return;
    const ipcError = normalizeIpcError('reveal_project_resource', error);
    logger.app.error(
      `Failed to reveal project resource code=${ipcError.code} incidentId=${ipcError.incidentId ?? 'none'}`,
      'SidebarResourceActions',
    );
    throw error;
  }
}

export async function renameWorksheetResource(
  worksheetPath: string,
  nextName: string,
): Promise<void> {
  await renameResource({ id: worksheetPath, kind: 'worksheet' }, nextName);
}
