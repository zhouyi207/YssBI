import type { WorkbenchPanelInfo } from '@/features/core/dockview/workbenchDockviewPort';
import { buildGraphLayoutTab } from '@/features/core/layout/layoutTabModel';
import { ensureEditorViewport, editorViewportScope } from '@/features/core/viewport';
import { logger } from '@/utils/appLogger';

import {
  isEditorOpenRejectionHandled,
  openEditorTab,
} from './openEditorTab';
import { switchEditorTab } from './switchEditorTab';

export interface OpenGraphInEditorOptions {
  /** `false` = preview tab (sidebar single-click). Default: pinned. */
  pinned?: boolean;
  /** Insert a newly opened editor at this TabBar index. */
  insertIndex?: number;
}

export async function openGraphInEditor(
  graphPath: string,
  name: string,
  type: 'event' | 'function',
  targetGroupId?: string,
  options?: OpenGraphInEditorOptions,
): Promise<WorkbenchPanelInfo | null> {
  logger.graph.trace(
    `openGraphInEditor called: path=${graphPath}, name=${name}, type=${type}`,
    'TabManagement',
  );

  const pinned = options?.pinned !== false;
  const tab = buildGraphLayoutTab(graphPath, type, { pinned });
  let panel: WorkbenchPanelInfo;
  try {
    panel = await openEditorTab(tab, {
      targetGroupId,
      pinned,
      insertIndex: options?.insertIndex,
    });
  } catch (error) {
    if (isEditorOpenRejectionHandled(error)) return null;
    throw error;
  }

  ensureEditorViewport(editorViewportScope(panel.groupId, graphPath));
  await switchEditorTab(panel.groupId, tab);
  return panel;
}
