import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { locateLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { isGraphLayoutTab, layoutTabResourceRef } from '@/features/core/layout/layoutTabModel';
import { getEditorGroupActiveTabId } from '@/features/core/layout/editorTabStore';
import { uiStore } from '@/features/core/ui/UIStore';
import { GraphService } from '@/services/graph/graphService';
import { logger } from '@/utils/appLogger';
import { unloadGraphDocument } from './graphDocumentUnload';
import { resolveTabDisplayName } from './resolveTabDisplayName';
import { clearDetailFocusForClosedTab } from '@/features/core/editor/detail/clearDetailFocusForClosedTab';
import { focusDetailOnActiveGraph } from '@/features/core/editor/detail/detailFocusCommands';
import { syncVariablesGraphScopeAfterClose } from '@/features/core/editor/detail/variablesGraphScope';
import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import { clearResourceDocumentState, isGraphResourceDirty, markResourceDirty } from '@/features/core/resource';
import { releaseEditorViewport } from '@/features/core/viewport';
import { editorViewportScope } from '@/features/core/viewport/viewportScope';
import { prepareActiveGroupBeforeLastTabClose } from '@/features/core/layout/editorGroupFocus';
import { activateCurrentEditorTab, activateEditorGroup } from './switchEditorTab';
import { deactivateGraphTab } from './activateGraphTab';
import {
  captureGraphSaveCommandContext,
  isGraphSaveCommandRevisionCurrent,
  type GraphSaveCommandContext,
} from '@/features/application/projectCommandContext';

async function restoreActiveGraphAfterClose(preferredNodeId: string): Promise<void> {
  const layoutStore = useLayoutStore.getState();
  const candidateGroupIds = [preferredNodeId, layoutStore.activeEditorGroupId]
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
      let context: GraphSaveCommandContext | undefined;
      try {
        context = captureGraphSaveCommandContext(effectivePath);
        await GraphService.saveProjectGraph(
          context.projectInstanceId,
          effectivePath,
          context.expectedRevision,
          context.operationId,
        );
        if (!isGraphSaveCommandRevisionCurrent(context, effectivePath)) return false;
        if (isGraphLayoutTab(located.tab)) {
          markResourceDirty({ id: effectivePath, kind: located.tab.type }, false);
        }
      } catch (error) {
        if (context && !context.isCurrent()) return false;
        uiStore.showToast(`保存失败：${error instanceof Error ? error.message : String(error)}`, 'error', 3000);
        return false;
      }
    }
  }

  const closingActiveTab = getEditorGroupActiveTabId(located.nodeId) === effectivePath;

  const nextGroupId = prepareActiveGroupBeforeLastTabClose(located.nodeId);
  if (nextGroupId) {
    await activateEditorGroup(nextGroupId);
  }

  useLayoutStore.getState().removeTab(located.nodeId, effectivePath);
  releaseEditorViewport(editorViewportScope(located.nodeId, effectivePath));
  if (closingActiveTab) {
    deactivateGraphTab(located.nodeId, effectivePath);
  }
  clearDetailFocusForClosedTab(effectivePath);
  syncVariablesGraphScopeAfterClose(effectivePath);
  if (!useEditorStore.getState().detailFocus) {
    focusDetailOnActiveGraph(located.nodeId);
  }
  await restoreActiveGraphAfterClose(located.nodeId);
  if (isGraphLayoutTab(located.tab)) {
    clearResourceDocumentState({ id: effectivePath, kind: located.tab.type });
  }
  void unloadGraphDocument(effectivePath).catch((error) => {
    logger.graph.warn(
      `Failed to release graph cache '${effectivePath}': ${error instanceof Error ? error.message : String(error)}`,
      'closeGraphTab',
    );
  });
  return true;
}
