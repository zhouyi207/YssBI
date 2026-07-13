import { locateLayoutTab } from '@/features/core/layout/layoutTabQueries';
import { layoutTabResourceRef } from '@/features/core/layout/layoutTabModel';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { isResourceDocumentDirty } from '@/features/core/resource';
import { uiStore } from '@/features/core/ui/UIStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { closeGraphTab } from './closeGraphTab';
import { clearDetailFocusForClosedTab } from '@/features/core/editor/detail/clearDetailFocusForClosedTab';
import { resolveTabDisplayName } from './resolveTabDisplayName';

export async function closeWorksheetTab(
  worksheetId: string,
  nodeId?: string,
  skipDirtyPrompt = false,
): Promise<boolean> {
  const located = locateLayoutTab(worksheetId, nodeId);
  if (!located?.tab) return false;

  if (isResourceDocumentDirty({ id: worksheetId, kind: 'worksheet' }) && !skipDirtyPrompt) {
    const displayName = resolveTabDisplayName(layoutTabResourceRef(located.tab), worksheetId);
    const shouldSave = await uiStore.confirm({
      title: '保存更改？',
      message: `“${displayName}” 已修改。关闭前是否保存？`,
      confirmText: '保存',
      cancelText: '不保存',
      type: 'info',
    });
    if (shouldSave) {
      try {
        await useWorksheetStore.getState().saveDocument(worksheetId);
      } catch (error) {
        uiStore.showToast(
          `保存失败：${error instanceof Error ? error.message : String(error)}`,
          'error',
          3000,
        );
        return false;
      }
    }
  }

  useLayoutStore.getState().removeTab(located.nodeId, worksheetId);
  clearDetailFocusForClosedTab(worksheetId);
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

export async function performWorksheetDelete(worksheetId: string): Promise<void> {
  await WorksheetService.deleteWorksheet(worksheetId);
  useWorksheetStore.getState().removeDocument(worksheetId);

  for (const node of Object.values(useLayoutStore.getState().nodes)) {
    if (node.data?.tabs?.some((tab) => tab.id === worksheetId)) {
      await closeWorksheetTab(worksheetId, node.id, true);
    }
  }
}

export async function deleteWorksheetWithConfirm(worksheetId: string): Promise<boolean> {
  const doc = useWorksheetStore.getState().documents[worksheetId];
  const name = doc?.name ?? worksheetId;
  const confirmed = await uiStore.confirm({
    title: '删除工作表',
    message: `确定要删除工作表「${name}」吗？`,
    confirmText: '删除',
    cancelText: '取消',
    type: 'danger',
  });
  if (!confirmed) return false;

  try {
    await performWorksheetDelete(worksheetId);
    return true;
  } catch (error) {
    uiStore.showToast(
      `删除失败：${error instanceof Error ? error.message : String(error)}`,
      'error',
      3000,
    );
    return false;
  }
}
