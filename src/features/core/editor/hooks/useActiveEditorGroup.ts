import { editorDockviewPort, useDockviewPortSnapshot } from '@/features/core/dockview';
import { DEFAULT_EDITOR_GROUP_ID } from '@/features/core/layout/workbenchLayoutDefaults';
import { useEditorGroupPlacement } from './useEditorGroupPlacement';
import { createGraphSelection } from '@/features/core/layout/layoutTabQueries';

export function useActiveEditorGroup(overrideGroupId?: string | null) {
  useDockviewPortSnapshot(editorDockviewPort);
  const focusedEditorGroupId = editorDockviewPort.getActiveGroupId() ?? null;
  const groupId = overrideGroupId ?? focusedEditorGroupId ?? DEFAULT_EDITOR_GROUP_ID;
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
