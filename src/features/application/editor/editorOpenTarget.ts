import { workbenchLayoutControl } from "@/modules/workbench/public";
import { workbenchLayoutRead } from "@/modules/workbench/public";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";

function groupContainsEditor(groupId: string): boolean {
  return workbenchLayoutRead
    .listGroupPanels(groupId)
    .some((panel) => panel.metadata.role === "editor");
}

export async function resolveEditorOpenTargetGroupId(
  explicitGroupId?: string | null,
): Promise<string> {
  const groupIds = new Set(workbenchLayoutRead.listGroups().map((group) => group.groupId));
  if (explicitGroupId && groupIds.has(explicitGroupId)) return explicitGroupId;

  const focused = useGraphSessionStore.getState().focusedSession;
  if (
    focused &&
    groupIds.has(focused.groupId) &&
    workbenchLayoutRead
      .findEditorPanelsByResource(focused.graphPath)
      .some((panel) => panel.groupId === focused.groupId)
  ) {
    return focused.groupId;
  }

  const recentGroupId = workbenchLayoutRead.getActiveEditorPanel()?.groupId ?? focused?.groupId;
  if (recentGroupId && groupIds.has(recentGroupId) && groupContainsEditor(recentGroupId)) {
    return recentGroupId;
  }

  return workbenchLayoutControl.ensureCentralGroup();
}
