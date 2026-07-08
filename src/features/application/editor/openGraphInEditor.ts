import { Graph } from '@/shared/types/domain';
import { buildGraphLayoutTab } from '@/features/core/layout/layoutTabModel';
import { resolveEditorTargetGroupId } from '@/features/core/layout/layoutTabQueries';
import { ensureGraphViewport } from '@/features/core/viewport';
import { logger } from '@/utils/appLogger';
import { openEditorTab } from './openEditorTab';
import { switchEditorGraphTab } from './switchEditorGraphTab';

export interface OpenGraphInEditorOptions {
  /** `false` = preview tab (sidebar single-click). Default: pinned. */
  pinned?: boolean;
}

export async function openGraphInEditor(
  graphPath: string,
  name: string,
  type: 'event' | 'function',
  targetGroupId?: string,
  initialData?: Graph,
  options?: OpenGraphInEditorOptions,
): Promise<void> {
  logger.graph.trace(`openGraphInEditor called: path=${graphPath}, name=${name}, type=${type}`, 'TabManagement');

  const pinned = options?.pinned !== false;
  openEditorTab(buildGraphLayoutTab(graphPath, name, type, { pinned }), { targetGroupId, pinned });
  const groupId = resolveEditorTargetGroupId(targetGroupId);
  const activated = await switchEditorGraphTab(groupId, graphPath, { id: graphPath, type });
  if (!activated) return;

  if (initialData?.canvas) {
    ensureGraphViewport(graphPath, initialData.canvas);
  }
}
