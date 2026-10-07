import type { WorkbenchEditorPanelInfo } from "@/modules/workbench/public";
import { ensureEditorViewport, editorViewportScope } from "@/features/core/viewport";
import { logger } from "@/utils/frontendLogger";

import { isEditorOpenRejectionHandled, openEditorPanel } from "./openEditorPanel";
import { revealActiveEditorDetails } from "./editorPanelActivation";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

export interface OpenGraphInEditorOptions {
  /** Insert a newly opened editor at this TabBar index. */
  insertIndex?: number;
}

export async function openGraphInEditor(
  graphPath: string,
  name: string,
  type: "event_graph" | "function_graph",
  targetGroupId?: string,
  options?: OpenGraphInEditorOptions,
): Promise<WorkbenchEditorPanelInfo | null> {
  const identity = captureProjectLifecycleState();
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
    if (!isProjectLifecycleStateCurrent(identity)) return null;
    if (isEditorOpenRejectionHandled(error)) return null;
    throw error;
  }

  if (!isProjectLifecycleStateCurrent(identity)) return null;
  ensureEditorViewport(editorViewportScope(panel.groupId, graphPath));
  if (!isProjectLifecycleStateCurrent(identity)) return null;
  await revealActiveEditorDetails(panel);
  return isProjectLifecycleStateCurrent(identity) ? panel : null;
}
