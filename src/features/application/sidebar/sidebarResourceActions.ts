import { logger } from '@/utils/appLogger';

import { ProjectService, type RevealProjectResourceRequest } from '@/services/project/projectService';
import { normalizeIpcError } from '@/services/ipc';
import { revealPath } from '@/services/platform/opener';

import { captureProjectCommandContext } from '@/features/application/projectCommandContext';
import { renameResource } from '@/features/application/resource/resourceActions';


export async function revealProjectResourceInExplorer(
  request: RevealProjectResourceRequest,
): Promise<void> {
  const context = captureProjectCommandContext();
  try {
    const path = await ProjectService.getProjectResourcePath(context.projectInstanceId, request);
    if (!context.isCurrent()) return;
    const result = await revealPath(path);
    if (!result.ok) throw new Error(result.failure.code);
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
