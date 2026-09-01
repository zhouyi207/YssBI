import type { WorkbenchEditorPanelInfo } from "@/features/core/dockview/workbenchRead";
import { ensureEditorViewport, editorViewportScope } from "@/features/core/viewport";
import { logger } from "@/features/application/observability/appLogger";
import { isValidGraphResourceTabId } from "@/shared/types/domain/graphResourcePath";

import { isEditorOpenRejectionHandled, openEditorPanel } from "./openEditorPanel";
import { activateEditorPanelAndSyncSession } from "./activateEditorPanelAndSyncSession";

export interface OpenGraphInEditorOptions {
  /** `false` = preview tab (sidebar single-click). Default: pinned. */
  pinned?: boolean;
  /** Insert a newly opened editor at this TabBar index. */
  insertIndex?: number;
}

export async function openGraphInEditor(
  graphPath: string,
  name: string,
  type: "event" | "function",
  targetGroupId?: string,
  options?: OpenGraphInEditorOptions,
): Promise<WorkbenchEditorPanelInfo | null> {
  logger.graph.trace(
    `openGraphInEditor called: path=${graphPath}, name=${name}, type=${type}`,
    "EditorPanelCommands",
  );

  if (!isValidGraphResourceTabId(graphPath, type)) {
    throw new Error(`Invalid graph resource path for ${type}: ${graphPath}`);
  }
  const pinned = options?.pinned !== false;
  const target = { resourceRef: graphPath, resourceKind: type, pinned } as const;
  let panel: WorkbenchEditorPanelInfo;
  try {
    panel = await openEditorPanel(target, {
      targetGroupId,
      insertIndex: options?.insertIndex,
    });
  } catch (error) {
    if (isEditorOpenRejectionHandled(error)) return null;
    throw error;
  }

  ensureEditorViewport(editorViewportScope(panel.groupId, graphPath));
  await activateEditorPanelAndSyncSession(panel);
  return panel;
}
