import { useMemo } from 'react';
import type { LayoutTab } from '@/shared/types';
import {
  editorDockviewPort,
  useDockviewPortSnapshot,
  useEditorPaneStateStore,
} from '@/features/core/dockview';

export interface EditorGroupPlacementSlice {
  tabIds: string[];
  activeTabId: string | null;
  selectedNodeIds: string[];
  selectedConnectionIds: string[];
  tabs: LayoutTab[];
}

function readLayoutTab(data: unknown): LayoutTab | null {
  return data && typeof data === 'object' ? data as LayoutTab : null;
}

/** Read-only projection of a Dockview group plus pane-local canvas selection. */
export function useEditorGroupPlacement(groupId: string): EditorGroupPlacementSlice {
  useDockviewPortSnapshot(editorDockviewPort);
  const group = editorDockviewPort.listGroups().find((candidate) => candidate.groupId === groupId);
  const panels = editorDockviewPort.listPanels().filter((panel) => panel.groupId === groupId);
  const activePanel = panels.find((panel) => panel.panelInstanceId === group?.activePanelInstanceId);
  const selection = useEditorPaneStateStore((state) => (
    activePanel ? state.selections[activePanel.panelInstanceId] : undefined
  ));

  return useMemo(() => {
    const tabs = panels
      .map((panel) => readLayoutTab(panel.tab?.data?.layoutTab))
      .filter((tab): tab is LayoutTab => tab !== null);
    return {
      tabIds: tabs.map((tab) => tab.id),
      activeTabId: activePanel?.tab?.resourceRef ?? null,
      selectedNodeIds: selection?.selectedNodeIds ?? [],
      selectedConnectionIds: selection?.selectedConnectionIds ?? [],
      tabs,
    };
  }, [activePanel, panels, selection]);
}
