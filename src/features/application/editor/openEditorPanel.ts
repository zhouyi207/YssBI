import { showWorkbenchLayoutError } from "@/modules/workbench/public";
import type { DetailFocus } from "@/features/core/editor/detail/detailTypes";
import type { EditorResourceTarget } from "@/modules/workbench/public";
import { workbenchDockviewControl } from "@/modules/workbench/public";
import {
  workbenchDockviewRead,
  type WorkbenchEditorPanelInfo,
  type WorkbenchPanelInfo,
} from "@/modules/workbench/public";
import { WorkbenchLayoutError } from "@/modules/workbench/public";

import { resolveEditorOpenTargetGroupId } from "./editorOpenTarget";
import { requestCloseEditorPanels } from "./editorPanelCloseCommands";
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

function markOpenRejectionHandled(error: Error): Error {
  handledOpenRejections.add(error);
  return error;
}

function handledPreviewCloseRejection(): Error {
  return markOpenRejectionHandled(new Error("editor preview close did not commit"));
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

async function replacePreviewPanel(
  target: EditorResourceTarget,
  targetGroupId: string,
): Promise<string> {
  const groupPanels = workbenchDockviewRead.listEditorPanelsInGroup(targetGroupId);
  const preview = groupPanels.find(
    (panel) => panel.metadata.resourceRef !== target.resourceRef && panel.metadata.pinned === false,
  );
  if (!preview) return targetGroupId;

  const wasSolePanel = groupPanels.length === 1;
  if (!(await requestCloseEditorPanels([preview.panelInstanceId]))) {
    throw handledPreviewCloseRejection();
  }
  return wasSolePanel ? resolveEditorOpenTargetGroupId(targetGroupId) : targetGroupId;
}

/** Open or activate an editor panel through the canonical workbench authority. */
export async function openEditorPanel(
  target: EditorResourceTarget,
  options?: OpenEditorPanelOptions,
): Promise<WorkbenchEditorPanelInfo> {
  const requestedPinned = target.pinned !== false;
  let panel: WorkbenchEditorPanelInfo;

  try {
    let targetGroupId = await resolveEditorOpenTargetGroupId(options?.targetGroupId);
    const existing = workbenchDockviewRead.findEditorPanelsByResource(target.resourceRef)[0];
    if (!requestedPinned && !existing) {
      targetGroupId = await replacePreviewPanel(target, targetGroupId);
    }

    const preserveExistingPreviewState = !requestedPinned && existing !== undefined;
    const pinned = preserveExistingPreviewState
      ? (existing.metadata.pinned ?? false)
      : requestedPinned;
    const sticky = preserveExistingPreviewState ? existing.metadata.sticky : target.sticky;
    const opened = await workbenchDockviewControl.openEditor({
      resourceRef: target.resourceRef,
      resourceKind: target.resourceKind,
      title: resolveResourceDisplayName(
        { id: target.resourceRef, kind: target.resourceKind },
        target.resourceRef,
      ),
      pinned,
      ...(sticky === undefined ? {} : { sticky }),
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
