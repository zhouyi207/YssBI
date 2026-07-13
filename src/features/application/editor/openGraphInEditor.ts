import { buildGraphLayoutTab } from '@/features/core/layout/layoutTabModel';
import { resolveEditorTargetGroupId } from '@/features/core/layout/layoutTabQueries';
import { logger } from '@/utils/appLogger';
import { openEditorTab } from './openEditorTab';
import { switchEditorTab } from './switchEditorTab';

export interface OpenGraphInEditorOptions {
  /** `false` = preview tab (sidebar single-click). Default: pinned. */
  pinned?: boolean;
  /** Insert or move to this index when dropped onto a TabBar. */
  insertIndex?: number;
}

export async function openGraphInEditor(
  graphPath: string,
  name: string,
  type: 'event' | 'function',
  targetGroupId?: string,
  options?: OpenGraphInEditorOptions,
): Promise<void> {
  logger.graph.trace(`openGraphInEditor called: path=${graphPath}, name=${name}, type=${type}`, 'TabManagement');

  const pinned = options?.pinned !== false;
  const tab = buildGraphLayoutTab(graphPath, type, { pinned });
  openEditorTab(tab, { targetGroupId, pinned, insertIndex: options?.insertIndex });
  const groupId = resolveEditorTargetGroupId(targetGroupId);
  const activated = await switchEditorTab(groupId, tab);
  if (!activated) return;
}
