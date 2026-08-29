import { useMemo } from 'react';
import { useEditorPaneStateStore } from '@/features/core/dockview/editorPaneStateStore';
import { useDockviewPortSnapshot } from '@/features/core/dockview/useDockviewPortSnapshot';
import { layoutTabFromEditorMetadata } from '@/features/core/dockview/workbenchPanelModel';
import { workbenchDockviewRead } from '@/features/core/dockview/workbenchRead';
import type { LayoutTab } from '@/shared/types';

export interface EditorGroupPlacementSlice {
  tabIds: string[];
  activeTabId: string | null;
  selectedNodeIds: string[];
  selectedConnectionIds: string[];
  tabs: LayoutTab[];
}

/** Read-only projection of a Dockview group plus pane-local canvas selection. */
export function useEditorGroupPlacement(
  groupId: string | null | undefined,
): EditorGroupPlacementSlice {
  useDockviewPortSnapshot(workbenchDockviewRead);
  const group = groupId
    ? workbenchDockviewRead.listGroups().find((candidate) => candidate.groupId === groupId)
    : undefined;
  const panels = groupId
    ? workbenchDockviewRead
      .listGroupPanels(groupId)
      .filter((panel) => panel.metadata.role === 'editor')
    : [];
  const activePanel = panels.find(
    (panel) => panel.panelInstanceId === group?.activePanelInstanceId,
  );
  const selection = useEditorPaneStateStore((state) => (
    activePanel ? state.selections[activePanel.panelInstanceId] : undefined
  ));

  return useMemo(() => {
    const tabs = panels.map((panel) => {
      if (panel.metadata.role !== 'editor') return null;
      return layoutTabFromEditorMetadata(panel.metadata);
    }).filter((tab): tab is LayoutTab => tab !== null);
    return {
      tabIds: tabs.map((tab) => tab.id),
      activeTabId: activePanel?.metadata.role === 'editor'
        ? activePanel.metadata.resourceRef
        : null,
      selectedNodeIds: selection?.selectedNodeIds ?? [],
      selectedConnectionIds: selection?.selectedConnectionIds ?? [],
      tabs,
    };
  }, [activePanel, panels, selection]);
}
