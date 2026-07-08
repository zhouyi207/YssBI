import { locateLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { isGraphLayoutTab, layoutTabResourceRef } from '@/features/core/layout/layoutTabModel';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { uiStore } from '@/features/core/ui/UIStore';
import { GraphService } from '@/services/graph/graphService';
import { releaseGraphCacheIfClosed } from './releaseGraphCache';
import { resolveTabDisplayName } from './resolveTabDisplayName';
import { clearDetailFocusForClosedTab } from '@/features/core/editor/detail/clearDetailFocusForClosedTab';
import { focusDetailOnActiveGraph } from '@/features/core/editor/detail/detailFocusCommands';
import { syncVariablesGraphScopeAfterClose } from '@/features/core/editor/detail/variablesGraphScope';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { clearResourceDocumentState, isGraphResourceDirty, markResourceDirty } from '@/features/core/resource';
import { deactivateGraphTab } from './activateGraphTab';
import { activateCurrentEditorTab } from './switchEditorTab';

async function restoreActiveGraphAfterClose(preferredNodeId: string): Promise<void> {
  const layoutStore = useLayoutStore.getState();
  const candidateGroupIds = [preferredNodeId, layoutStore.activeEditorGroupId, layoutStore.activeGroupId]
    .filter((id): id is string => Boolean(id));
  for (const groupId of candidateGroupIds) {
    const activated = await activateCurrentEditorTab(groupId);
    if (activated) return;
  }
}

export async function closeGraphTab(graphPath: string, nodeId?: string, skipDirtyPrompt = false): Promise<boolean> {
  const located = locateLayoutTab(graphPath, nodeId);
  if (!located?.tab) return false;

  let effectivePath = graphPath;
  const graphKind =
    located.tab.type === 'event' || located.tab.type === 'function' ? located.tab.type : null;
  if (graphKind && isGraphResourceDirty(effectivePath, graphKind) && !skipDirtyPrompt) {
    const displayName = resolveTabDisplayName(layoutTabResourceRef(located.tab), effectivePath);
    const shouldSave = await uiStore.confirm({
      title: '保存更改？',
      message: `“${displayName}” 已修改。关闭前是否保存？`,
      confirmText: '保存',
      cancelText: '不保存',
      type: 'info',
    });
    if (shouldSave) {
      try {
        const savedPath = await GraphService.saveProjectGraph(effectivePath);
        if (isGraphLayoutTab(located.tab)) {
          markResourceDirty({ id: savedPath, kind: located.tab.type }, false);
        }
        effectivePath = savedPath;
      } catch (error) {
        uiStore.showToast(`保存失败：${error instanceof Error ? error.message : String(error)}`, 'error', 3000);
        return false;
      }
    }
  }

  useLayoutStore.getState().removeTab(located.nodeId, effectivePath);
  deactivateGraphTab(located.nodeId);
  clearDetailFocusForClosedTab(effectivePath, located.tab.type);
  syncVariablesGraphScopeAfterClose(effectivePath);
  if (!useEditorStore.getState().detailFocus) {
    focusDetailOnActiveGraph(located.nodeId);
  }
  await restoreActiveGraphAfterClose(located.nodeId);
  if (isGraphLayoutTab(located.tab)) {
    clearResourceDocumentState({ id: effectivePath, kind: located.tab.type });
  }
  releaseGraphCacheIfClosed(effectivePath);
  return true;
}
