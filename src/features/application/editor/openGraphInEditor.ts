import type { WorkbenchEditorPanelInfo } from "@/modules/workbench/public";
import { ensureEditorViewport, editorViewportScope } from "@/features/core/viewport";
import { logger } from "@/features/application/observability/appLogger";

import { isEditorOpenRejectionHandled, openEditorPanel } from "./openEditorPanel";
import { activateEditorPanelAndSyncSession } from "./activateEditorPanelAndSyncSession";

export interface OpenGraphInEditorOptions {
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

  const target = { resourceRef: graphPath, resourceKind: type } as const;
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
