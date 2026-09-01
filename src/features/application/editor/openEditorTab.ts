import { showWorkbenchLayoutError } from "@/features/application/layout/workbenchLayoutErrorFeedback";
import type { DetailFocus } from "@/shared/types/ui/detail";
import type { EditorResourceKind } from "@/features/core/dockview/workbenchPanelModel";
import { workbenchDockviewControl } from "@/features/core/dockview/workbenchControl";
import {
  workbenchDockviewRead,
  type WorkbenchPanelInfo,
} from "@/features/core/dockview/workbenchRead";
import { WorkbenchLayoutError } from "@/features/core/dockview/workbenchTypes";
import type { LayoutTab } from "@/shared/types";

import { resolveEditorOpenTargetGroupId } from "./editorOpenTarget";
import { requestCloseWorkbenchPanels } from "./workbenchPanelClose";
import { resolveTabDisplayName } from "./resolveTabDisplayName";
import { revealDetails } from "./rightSidebarActions";

export interface OpenEditorTabOptions {
  targetGroupId?: string;
  insertIndex?: number;
  focusDetail?: DetailFocus;
  /** `false` opens in the preview slot. Default: pinned. */
  pinned?: boolean;
}

const handledOpenRejections = new WeakSet<Error>();

function editorResourceKind(tab: LayoutTab): EditorResourceKind {
  if (tab.type === "event" || tab.type === "function" || tab.type === "worksheet") {
    return tab.type;
  }
  throw new WorkbenchLayoutError("invalid_panel_metadata");
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

async function replacePreviewPanel(tab: LayoutTab, targetGroupId: string): Promise<string> {
  const groupPanels = workbenchDockviewRead.listGroupPanels(targetGroupId);
  const preview = groupPanels.find(
    (panel) =>
      panel.metadata.role === "editor" &&
      panel.metadata.resourceRef !== tab.id &&
      panel.metadata.pinned === false,
  );
  if (!preview) return targetGroupId;

  const wasSolePanel = groupPanels.length === 1;
  if (!(await requestCloseWorkbenchPanels([preview.panelInstanceId]))) {
    throw handledPreviewCloseRejection();
  }
  return wasSolePanel ? resolveEditorOpenTargetGroupId(targetGroupId) : targetGroupId;
}

/** Open or activate an editor panel through the canonical workbench authority. */
export async function openEditorTab(
  tab: LayoutTab,
  options?: OpenEditorTabOptions,
): Promise<WorkbenchPanelInfo> {
  const requestedPinned = options?.pinned !== false;
  let panel: WorkbenchPanelInfo;

  try {
    let targetGroupId = await resolveEditorOpenTargetGroupId(options?.targetGroupId);
    const existing = workbenchDockviewRead.findEditorPanelsByResource(tab.id)[0];
    if (!requestedPinned && !existing) {
      targetGroupId = await replacePreviewPanel(tab, targetGroupId);
    }

    const preserveExistingPreviewState = !requestedPinned && existing?.metadata.role === "editor";
    const pinned = preserveExistingPreviewState
      ? (existing.metadata.pinned ?? false)
      : requestedPinned;
    const sticky = preserveExistingPreviewState ? existing.metadata.sticky : tab.sticky;
    const resourceKind = editorResourceKind(tab);
    panel = await workbenchDockviewControl.openEditor({
      resourceRef: tab.id,
      resourceKind,
      title: resolveTabDisplayName({ id: tab.id, kind: resourceKind }, tab.id),
      pinned,
      ...(sticky === undefined ? {} : { sticky }),
      targetGroupId,
      index: options?.insertIndex,
      mode: "reuse-resource",
    });
  } catch (error) {
    throw presentOpenRejection(error);
  }

  if (options?.focusDetail) await revealDetails(options.focusDetail);
  return panel;
}
