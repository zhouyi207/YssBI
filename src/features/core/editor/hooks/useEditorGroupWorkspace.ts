import { useContext, useMemo } from 'react';
import { editorDockviewPort, useDockviewPortSnapshot } from '@/features/core/dockview';
import { DEFAULT_EDITOR_GROUP_ID } from '@/features/core/layout/workbenchLayoutDefaults';
import { createGraphSelection } from '@/features/core/layout/layoutTabQueries';
import { GroupContext } from '../context/GroupContext';
import { useEditorGroupPlacement } from './useEditorGroupPlacement';

export function useEditorGroupWorkspace(overrideGroupId?: string | null) {
  const contextGroupId = useContext(GroupContext);
  useDockviewPortSnapshot(editorDockviewPort);
  const groupId = overrideGroupId
    ?? contextGroupId
    ?? editorDockviewPort.getActiveGroupId()
    ?? DEFAULT_EDITOR_GROUP_ID;
  const placement = useEditorGroupPlacement(groupId);

  return useMemo(() => ({
    groupId,
    tabs: placement.tabs,
    activeTabId: placement.activeTabId,
    selectedNodeIds: placement.selectedNodeIds,
    selectedConnectionIds: placement.selectedConnectionIds,
    selection: createGraphSelection(placement.selectedNodeIds, placement.selectedConnectionIds),
  }), [groupId, placement]);
}
