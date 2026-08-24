import { useContext, useMemo } from 'react';
import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';
import { useDockviewPortSnapshot } from '@/features/core/dockview/useDockviewPortSnapshot';
import { createGraphSelection } from '@/features/core/layout/layoutTabQueries';
import { GroupContext } from '../context/GroupContext';
import { useEditorGroupPlacement } from './useEditorGroupPlacement';

export function useEditorGroupWorkspace(overrideGroupId?: string | null) {
  const contextGroupId = useContext(GroupContext);
  useDockviewPortSnapshot(workbenchDockviewPort);
  const groupId = overrideGroupId
    ?? contextGroupId
    ?? workbenchDockviewPort.getActiveEditorPanel()?.groupId
    ?? null;
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
