import {
  getActiveLayoutTabAmongGroups,
  locateLayoutTab,
} from '@/features/core/layout/layoutTabQueries';
import type { LayoutTab } from '@/shared/types';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { uiStore } from '@/features/core/ui/UIStore';
import { useGraphDataStore, useProjectIOStore } from '@/features/core/dataStore';
import { GraphService } from '@/services/graph/graphService';
import { releaseGraphCacheIfClosed } from './releaseGraphCache';
import { clearDetailFocusForClosedTab } from '@/features/core/editor/detail/clearDetailFocusForClosedTab';
import { focusDetailOnActiveGraph } from '@/features/core/editor/detail/detailFocusCommands';
import { syncVariablesGraphScopeAfterClose } from '@/features/core/editor/detail/variablesGraphScope';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { clearResourceDocumentState, markResourceDirty } from '@/features/core/resource';

function isGraphTab(tab: LayoutTab | undefined): tab is LayoutTab & { type: 'event' | 'function' } {
  return tab?.type === 'event' || tab?.type === 'function';
}

async function restoreActiveGraphAfterClose(preferredNodeId: string): Promise<void> {
  const layoutStore = useLayoutStore.getState();
  const activeTab = getActiveLayoutTabAmongGroups(
    [preferredNodeId, layoutStore.activeEditorGroupId, layoutStore.activeGroupId].filter(
      (id): id is string => Boolean(id),
    ),
    layoutStore.nodes,
  );

  if (!isGraphTab(activeTab)) return;
  if (useGraphDataStore.getState().hasGraph(activeTab.id)) return;
  await useProjectIOStore.getState().loadGraph(activeTab.id);
}

export async function closeGraphTab(graphId: string, nodeId?: string, skipDirtyPrompt = false): Promise<boolean> {
  const located = locateLayoutTab(graphId, nodeId);
  if (!located?.tab) return false;

  if (located.tab.isDirty && !skipDirtyPrompt) {
    const shouldSave = await uiStore.confirm({
      title: '保存更改？',
      message: `“${located.tab.title}” 已修改。关闭前是否保存？`,
      confirmText: '保存',
      cancelText: '不保存',
      type: 'info',
    });
    if (shouldSave) {
      try {
        await GraphService.saveProjectGraph(graphId);
        if (located.tab.type === 'event' || located.tab.type === 'function') {
          markResourceDirty({ id: graphId, kind: located.tab.type }, false);
        } else {
          useLayoutStore.getState().setTabDirty(graphId, false);
        }
      } catch (error) {
        uiStore.showToast(`保存失败：${error instanceof Error ? error.message : String(error)}`, 'error', 3000);
        return false;
      }
    }
  }

  useLayoutStore.getState().removeTab(located.nodeId, graphId);
  clearDetailFocusForClosedTab(graphId, located.tab.type);
  syncVariablesGraphScopeAfterClose(graphId);
  if (!useEditorStore.getState().detailFocus) {
    focusDetailOnActiveGraph(located.nodeId);
  }
  await restoreActiveGraphAfterClose(located.nodeId);
  if (located.tab.type === 'event' || located.tab.type === 'function') {
    clearResourceDocumentState({ id: graphId, kind: located.tab.type });
  }
  releaseGraphCacheIfClosed(graphId);
  return true;
}
