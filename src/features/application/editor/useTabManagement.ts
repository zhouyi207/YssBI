import { useCallback } from 'react';
import { Graph } from '@/shared/types/domain';
import { openGraphInEditor } from './openGraphInEditor';
import { getGraphByPath } from '@/features/core/dataStore';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { getActiveLayoutTab, resolveEditorGroupId } from '@/features/core/layout/layoutTabQueries';
import { releaseGraphCacheIfClosed } from './releaseGraphCache';
import { closeEditorTab } from './closeEditorTab';
import { ensureGraphViewport } from '@/features/core/viewport';
import { logger } from '@/utils/appLogger';
import { applyEditorTabSelection } from './editorTabSelection';
import { switchEditorGraphTab } from './switchEditorGraphTab';

/**
 * Tab Management Hook
 * Handles opening, closing, and switching between tabs
 */
export function useTabManagement() {
  const handleSetActiveTabId = useCallback((
    newId: string | null,
    forceType?: 'event' | 'function' | 'setting',
    initialData?: Graph,
    targetGroupId?: string
  ) => {
    logger.graph.trace(`handleSetActiveTabId called: newId=${newId}, forceType=${forceType}, targetGroupId=${targetGroupId}`, 'TabManagement');

    const groupId = resolveEditorGroupId(targetGroupId);
    if (groupId) applyEditorTabSelection(groupId, newId);
    if (!newId) return;

    const resolvedType = forceType || (initialData as { type?: string })?.type
      || (getGraphByPath(newId) as { type?: string })?.type;

    if (resolvedType === 'event' || resolvedType === 'function') {
      void switchEditorGraphTab(groupId!, newId);
    } else {
      const tabSource = initialData || getGraphByPath(newId);
      ensureGraphViewport(newId, tabSource?.canvas);
    }

    logger.graph.trace(`handleSetActiveTabId final type: ${resolvedType}`, 'TabManagement');
  }, []);

  const activateTab = useCallback((id: string | null, targetGroupId?: string) => {
    handleSetActiveTabId(id, undefined, undefined, targetGroupId);
  }, [handleSetActiveTabId]);

  const openGraph = useCallback(async (
    id: string,
    name: string,
    type: "event" | "function",
    initialData?: Graph
  ) => {
    await openGraphInEditor(id, name, type, undefined, initialData);
  }, []);

  const openSettingsTab = useCallback(() => {
    const layoutStore = useLayoutStore.getState();
    const targetGroupId = layoutStore.activeEditorGroupId || 'default_editor';
    layoutStore.openSettings();
    handleSetActiveTabId("settings", "setting", undefined, targetGroupId);
  }, [handleSetActiveTabId]);

  const closeTab = useCallback((id: string, e?: React.MouseEvent, options?: { skipDirtyPrompt?: boolean }) => {
    if (e) e.stopPropagation();
    void closeEditorTab(id, undefined, options?.skipDirtyPrompt);
  }, []);

  const splitEditorRight = useCallback((sourceGroupId: string) => {
    const nodes = useLayoutStore.getState().nodes;
    const activeTab = getActiveLayoutTab(sourceGroupId, nodes)?.tab;
    useLayoutStore.getState().splitNode(sourceGroupId, 'row', activeTab?.component || 'GraphEditor');
  }, []);

  const closeGroup = useCallback(async (id: string) => {
    const tabIds = useLayoutStore.getState().nodes[id]?.data?.tabs?.map((tab) => tab.id) ?? [];
    for (const tabId of tabIds) {
      const closed = await closeEditorTab(tabId, id);
      if (!closed) return;
    }
    useLayoutStore.getState().removeNode(id);
    tabIds.forEach(releaseGraphCacheIfClosed);
  }, []);

  return {
    setActiveTabId: activateTab,
    openGraph,
    openSettingsTab,
    closeTab,
    splitEditorRight,
    closeGroup,
  };
}
