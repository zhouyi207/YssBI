import { locateLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { layoutTabResourceRef } from '@/features/core/layout/layoutTabModel';

import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { isResourceDocumentDirty } from '@/features/core/resource';
import { uiStore } from '@/features/core/ui/UIStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import {
  captureProjectCommandContext,
  type ProjectCommandContext,
} from '@/features/application/projectCommandContext';

import { closeGraphTab } from './closeGraphTab';
import { clearDetailFocusForClosedTab } from '@/features/core/editor/detail/clearDetailFocusForClosedTab';
import { resolveTabDisplayName } from './resolveTabDisplayName';

export async function closeWorksheetTab(
  worksheetPath: string,
  nodeId?: string,
  skipDirtyPrompt = false,
): Promise<boolean> {
  const located = locateLayoutTab(worksheetPath, nodeId);
  if (!located?.tab) return false;

  if (isResourceDocumentDirty({ id: worksheetPath, kind: 'worksheet' }) && !skipDirtyPrompt) {
    const displayName = resolveTabDisplayName(layoutTabResourceRef(located.tab), worksheetPath);
    const context = captureProjectCommandContext();
    const shouldSave = await uiStore.confirm({
      title: '保存更改？',
      message: `“${displayName}” 已修改。关闭前是否保存？`,
      confirmText: '保存',
      cancelText: '不保存',
      type: 'info',
    });
    if (!context.isCurrent()) return false;
    if (shouldSave) {
      try {
        const saved = await useWorksheetStore.getState().saveDocument(worksheetPath);
        if (!saved || !context.isCurrent()) return false;
      } catch (error) {
        if (!context.isCurrent()) return false;
        uiStore.showToast(
          `保存失败：${error instanceof Error ? error.message : String(error)}`,
          'error',
          3000,
        );
        return false;
      }
    }
  }

  useLayoutStore.getState().removeTab(located.nodeId, worksheetPath);
  clearDetailFocusForClosedTab(worksheetPath);
  return true;
}

export async function closeEditorTab(
  tabId: string,
  nodeId?: string,
  skipDirtyPrompt = false,
): Promise<boolean> {
  const located = locateLayoutTab(tabId, nodeId);

  const tabType = located?.tab?.type;
  if (tabType === 'worksheet') {
    return closeWorksheetTab(tabId, nodeId, skipDirtyPrompt);
  }
  if (tabType === 'event' || tabType === 'function') {
    return closeGraphTab(tabId, nodeId, skipDirtyPrompt);
  }

  if (located?.nodeId) {
    useLayoutStore.getState().removeTab(located.nodeId, tabId);
    return true;
  }
  return false;
}

export async function performWorksheetDelete(
  worksheetPath: string,
  context: ProjectCommandContext = captureProjectCommandContext(),
): Promise<boolean> {
  const document = useWorksheetStore.getState().documents[worksheetPath]
    ?? await WorksheetService.loadWorksheet(context.projectInstanceId, worksheetPath);
  if (!context.isCurrent()) return false;
  const committed = await WorksheetService.removeWorksheet(
    context.projectInstanceId,
    context.operationId,
    worksheetPath,
    document.revision,
  );
  if (!context.isCurrent()) return false;
  await projectPublicationCoordinator.submit({ result: committed });
  return context.isCurrent();
}

export async function deleteWorksheetWithConfirm(worksheetPath: string): Promise<boolean> {
  const name = resolveTabDisplayName({ id: worksheetPath, kind: 'worksheet' }, worksheetPath);
  const context = captureProjectCommandContext();
  const confirmed = await uiStore.confirm({
    title: '删除工作表',
    message: `确定要删除工作表「${name}」吗？`,
    confirmText: '删除',
    cancelText: '取消',
    type: 'danger',
  });
  if (!confirmed || !context.isCurrent()) return false;

  try {
    return await performWorksheetDelete(worksheetPath, context);
  } catch (error) {
    if (!context.isCurrent()) return false;
    uiStore.showToast(
      `删除失败：${error instanceof Error ? error.message : String(error)}`,
      'error',
      3000,
    );
    return false;
  }
}
