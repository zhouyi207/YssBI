import type { EditorSplitEdge } from './editorSplitLayout';
import type { LayoutTab } from '@/shared/types/ui';
import { splitComponentForTab } from './layoutTabModel';
import { useLayoutStore } from './layoutStore';
import { getActiveLayoutTab } from './layoutTabQueries';
import { persistEditorGridDebounced, collapseEditorGroupsForProjectSwitch } from './workbenchLayoutService';

/** VS Code IEditorGroupsService — thin facade over layoutStore editor grid actions. */
export const EditorGroupsService = {
  getActiveGroupId(): string | null {
    return useLayoutStore.getState().activeEditorGroupId;
  },

  setActiveGroup(groupId: string | null): void {
    useLayoutStore.getState().setActiveGroup(groupId);
  },

  splitGroupAtEdge(
    targetGroupId: string,
    edge: EditorSplitEdge,
    payload: {
      component: string;
      tabs: LayoutTab[];
      activeTabId?: string;
      pinSourceActiveTab?: boolean;
    },
  ): string | null {
    const created = useLayoutStore.getState().splitEditorGroupAtEdge(targetGroupId, edge, payload);
    if (created) persistEditorGridDebounced();
    return created;
  },

  splitActiveTabRight(groupId: string): string | null {
    const nodes = useLayoutStore.getState().nodes;
    const activeTab = getActiveLayoutTab(groupId, nodes)?.tab;
    return EditorGroupsService.splitGroupAtEdge(groupId, 'right', {
      component: splitComponentForTab(activeTab),
      tabs: activeTab ? [{ ...activeTab, pinned: true as const }] : [],
      activeTabId: activeTab?.id,
      pinSourceActiveTab: true,
    });
  },

  splitActiveTabDown(groupId: string): string | null {
    const nodes = useLayoutStore.getState().nodes;
    const activeTab = getActiveLayoutTab(groupId, nodes)?.tab;
    return EditorGroupsService.splitGroupAtEdge(groupId, 'bottom', {
      component: splitComponentForTab(activeTab),
      tabs: activeTab ? [{ ...activeTab, pinned: true as const }] : [],
      activeTabId: activeTab?.id,
      pinSourceActiveTab: true,
    });
  },

  moveTab(
    sourceGroupId: string,
    tabId: string,
    targetGroupId: string,
    targetTabIndex?: number,
  ): void {
    useLayoutStore.getState().moveTab(sourceGroupId, tabId, targetGroupId, targetTabIndex);
    useLayoutStore.getState().setActiveGroup(targetGroupId);
    persistEditorGridDebounced();
  },

  setActiveTab(groupId: string, tabId: string | null): void {
    useLayoutStore.getState().setEditorGroupActiveTab(groupId, tabId);
    useLayoutStore.getState().setActiveGroup(groupId);
  },

  collapseToSingleGroup(): void {
    collapseEditorGroupsForProjectSwitch();
  },

  toggleMaximizeGroup(groupId: string): void {
    useLayoutStore.getState().toggleMaximizeEditorGroup(groupId);
    persistEditorGridDebounced();
  },

  getGridSnapshot() {
    const state = useLayoutStore.getState();
    return {
      activeEditorGroupId: state.activeEditorGroupId,
      nodes: state.nodes,
    };
  },
};
