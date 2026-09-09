import { showWorkbenchLayoutError } from "@/modules/workbench/public";
import type { DetailFocus } from "@/features/core/editor/detail/detailTypes";
import type { EditorResourceTarget } from "@/modules/workbench/public";
import { workbenchDockviewControl } from "@/modules/workbench/public";
import { type WorkbenchEditorPanelInfo, type WorkbenchPanelInfo } from "@/modules/workbench/public";
import { WorkbenchLayoutError } from "@/modules/workbench/public";

import { resolveEditorOpenTargetGroupId } from "./editorOpenTarget";
import { resolveResourceDisplayName } from "./resolveResourceDisplayName";
import { revealDetails } from "./rightSidebarActions";

export interface OpenEditorPanelOptions {
  targetGroupId?: string;
  insertIndex?: number;
  focusDetail?: DetailFocus;
}

const handledOpenRejections = new WeakSet<Error>();

function isEditorPanelInfo(panel: WorkbenchPanelInfo): panel is WorkbenchEditorPanelInfo {
  return panel.metadata.role === "editor";
}

function presentOpenRejection(error: unknown): Error {
  const rejection = error instanceof Error ? error : new WorkbenchLayoutError("panel_open_failed");
  if (!handledOpenRejections.has(rejection)) {
    showWorkbenchLayoutError(rejection);
    handledOpenRejections.add(rejection);
  }
  return rejection;
}

export function isEditorOpenRejectionHandled(error: unknown): boolean {
  return error instanceof Error && handledOpenRejections.has(error);
}

/** Open or activate an editor panel through the canonical workbench authority. */
export async function openEditorPanel(
  target: EditorResourceTarget,
  options?: OpenEditorPanelOptions,
): Promise<WorkbenchEditorPanelInfo> {
  let panel: WorkbenchEditorPanelInfo;

  try {
    const targetGroupId = await resolveEditorOpenTargetGroupId(options?.targetGroupId);
    const opened = await workbenchDockviewControl.openEditor({
      resourceRef: target.resourceRef,
      resourceKind: target.resourceKind,
      title: resolveResourceDisplayName(
        { id: target.resourceRef, kind: target.resourceKind },
        target.resourceRef,
      ),
      pinned: true,
      ...(target.sticky === undefined ? {} : { sticky: target.sticky }),
      targetGroupId,
      index: options?.insertIndex,
      mode: "reuse-resource",
    });
    if (!isEditorPanelInfo(opened)) {
      throw new WorkbenchLayoutError("invalid_panel_metadata");
    }
    panel = opened;
  } catch (error) {
    throw presentOpenRejection(error);
  }

  if (options?.focusDetail) await revealDetails(options.focusDetail);
  return panel;
}
