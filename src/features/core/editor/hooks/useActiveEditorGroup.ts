import { workbenchDockviewRead } from '@/features/core/dockview/workbenchRead';
import { useDockviewPortSnapshot } from '@/features/core/dockview/useDockviewPortSnapshot';
import { createGraphSelection } from '@/features/core/layout/layoutTabQueries';
import { useEditorGroupPlacement } from './useEditorGroupPlacement';

export function useActiveEditorGroup(overrideGroupId?: string | null) {
  useDockviewPortSnapshot(workbenchDockviewRead);
  const focusedEditorGroupId = workbenchDockviewRead.getActiveEditorPanel()?.groupId ?? null;
  const groupId = overrideGroupId ?? focusedEditorGroupId;
  const placement = useEditorGroupPlacement(groupId);

  return {
    groupId,
    focusedEditorGroupId,
    activeTabId: placement.activeTabId,
    tabs: placement.tabs,
    selectedNodeIds: placement.selectedNodeIds,
    selectedConnectionIds: placement.selectedConnectionIds,
    selection: createGraphSelection(placement.selectedNodeIds, placement.selectedConnectionIds),
    node: undefined,
  };
}
