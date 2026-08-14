import { useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import type { LayoutTab } from '@/shared/types';
import {
  type EditorTabState,
  useEditorTabStore,
} from '@/features/core/layout/editorTabStore';

const EMPTY_TAB_IDS: string[] = [];
const EMPTY_SELECTED: string[] = [];
const EMPTY_SELECTED_CONNECTIONS: string[] = [];
const EMPTY_PLACEMENT = {
  tabIds: EMPTY_TAB_IDS,
  activeTabId: null as string | null,
  selectedNodeIds: EMPTY_SELECTED,
  selectedConnectionIds: EMPTY_SELECTED_CONNECTIONS,
};
export const EMPTY_GROUP_TABS: LayoutTab[] = [];

function resolveGroupTabs(state: EditorTabState, tabIds: readonly string[]): LayoutTab[] {
  if (tabIds.length === 0) return EMPTY_GROUP_TABS;
  const tabs: LayoutTab[] = [];
  for (const tabId of tabIds) {
    const tab = state.registry[tabId];
    if (tab) tabs.push(tab);
  }
  return tabs.length > 0 ? tabs : EMPTY_GROUP_TABS;
}

export interface EditorGroupPlacementSlice {
  tabIds: string[];
  activeTabId: string | null;
  selectedNodeIds: string[];
  selectedConnectionIds: string[];
  tabs: LayoutTab[];
}

/**
 * Narrow per-group placement subscription.
 * Avoids selecting the full registry object (cross-group fan-out + unstable snapshots).
 */
export function useEditorGroupPlacement(groupId: string): EditorGroupPlacementSlice {
  const placement = useEditorTabStore(
    useShallow((state) => {
      const p = state.placements[groupId];
      if (!p) return EMPTY_PLACEMENT;
      return {
        tabIds: p.tabIds,
        activeTabId: p.activeTabId,
        selectedNodeIds: p.selectedNodeIds,
        selectedConnectionIds: p.selectedConnectionIds,
      };
    }),
  );

  const tabs = useEditorTabStore(
    useShallow((state) => resolveGroupTabs(state, placement.tabIds)),
  );

  return useMemo(
    () => ({
      tabIds: placement.tabIds,
      activeTabId: placement.activeTabId,
      selectedNodeIds: placement.selectedNodeIds,
      selectedConnectionIds: placement.selectedConnectionIds,
      tabs,
    }),
    [
      placement.tabIds,
      placement.activeTabId,
      placement.selectedNodeIds,
      placement.selectedConnectionIds,
      tabs,
    ],
  );
}
