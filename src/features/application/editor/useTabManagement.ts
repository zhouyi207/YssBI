import { useCallback } from 'react';
import { Graph } from '@/shared/types/domain';
import { openGraphInEditor } from './openGraphInEditor';
import { getActiveLayoutTab, locateLayoutTab, resolveEditorGroupId } from '@/features/core/layout/layoutTabQueries';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { logger } from '@/utils/appLogger';
import { applyEditorTabSelection } from './editorTabSelection';
import {
  closeEditorGroup,
  closeTab as closeTabCommand,
  splitEditorGroup,
  switchTab,
} from './tabCommands';

/**
 * Tab Management Hook — thin React facade over tabCommands.
 */
export function useTabManagement() {
  const handleSetActiveTabId = useCallback((
    newId: string | null,
    _forceType?: 'event' | 'function' | 'setting',
    _initialData?: Graph,
    targetGroupId?: string,
  ) => {
    logger.graph.trace(`handleSetActiveTabId called: newId=${newId}, targetGroupId=${targetGroupId}`, 'TabManagement');

    const groupId = resolveEditorGroupId(targetGroupId);
    if (groupId) applyEditorTabSelection(groupId, newId);
    if (!newId || !groupId) return;

    void switchTab(groupId, newId);
  }, []);

  const activateTab = useCallback((id: string | null, targetGroupId?: string) => {
    handleSetActiveTabId(id, undefined, undefined, targetGroupId);
  }, [handleSetActiveTabId]);

  const openGraph = useCallback(async (
    id: string,
    name: string,
    type: "event" | "function",
    options?: { initialData?: Graph; pinned?: boolean; targetGroupId?: string },
  ) => {
    await openGraphInEditor(
      id,
      name,
      type,
      options?.targetGroupId,
      options?.initialData,
      { pinned: options?.pinned },
    );
  }, []);

  const openSettingsTab = useCallback(() => {
    const layoutStore = useLayoutStore.getState();
    const targetGroupId = layoutStore.activeEditorGroupId || 'default_editor';
    layoutStore.openSettings();
    handleSetActiveTabId("settings", "setting", undefined, targetGroupId);
  }, [handleSetActiveTabId]);

  /** Close by tab id (keyboard / global); resolves owning editor group. */
  const closeTab = useCallback((tabId: string, e?: React.MouseEvent, options?: { skipDirtyPrompt?: boolean }) => {
    if (e) e.stopPropagation();
    const located = locateLayoutTab(tabId);
    if (!located) return;
    void closeTabCommand(located.nodeId, tabId, options?.skipDirtyPrompt);
  }, []);

  const splitEditorRight = useCallback((sourceGroupId: string) => {
    splitEditorGroup(sourceGroupId, 'row');
  }, []);

  const closeGroup = useCallback(async (groupId: string) => {
    await closeEditorGroup(groupId);
  }, []);

  return {
    setActiveTabId: activateTab,
    openGraph,
    openSettingsTab,
    closeTab,
    splitEditorRight,
    closeGroup,
    switchTab,
    splitEditorGroup,
    closeEditorGroup,
    getActiveLayoutTab,
  };
}
