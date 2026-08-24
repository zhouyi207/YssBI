import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';
import { useGraphSessionStore } from '@/features/core/graphSession/graphSessionStore';

function groupContainsEditor(groupId: string): boolean {
  return workbenchDockviewPort
    .listGroupPanels(groupId)
    .some((panel) => panel.metadata.role === 'editor');
}

export async function resolveEditorOpenTargetGroupId(
  explicitGroupId?: string | null,
): Promise<string> {
  const groupIds = new Set(
    workbenchDockviewPort.listGroups().map((group) => group.groupId),
  );
  if (explicitGroupId && groupIds.has(explicitGroupId)) return explicitGroupId;

  const focused = useGraphSessionStore.getState().focusedSession;
  if (
    focused
    && groupIds.has(focused.groupId)
    && workbenchDockviewPort
      .findEditorPanelsByResource(focused.graphPath)
      .some((panel) => panel.groupId === focused.groupId)
  ) {
    return focused.groupId;
  }

  const recentGroupId = workbenchDockviewPort.getActiveEditorPanel()?.groupId
    ?? focused?.groupId;
  if (
    recentGroupId
    && groupIds.has(recentGroupId)
    && groupContainsEditor(recentGroupId)
  ) {
    return recentGroupId;
  }

  return workbenchDockviewPort.ensureCentralGroup();
}
