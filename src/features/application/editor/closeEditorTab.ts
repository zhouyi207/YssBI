import type { LayoutTab } from '@/shared/types';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { uiStore } from '@/features/core/ui/UIStore';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { closeGraphTab } from './closeGraphTab';

function findTab(tabId: string): { nodeId: string; tab: LayoutTab } | null {
  for (const node of Object.values(useLayoutStore.getState().nodes)) {
    const tab = node.data?.tabs?.find((item) => item.id === tabId);
    if (tab) return { nodeId: node.id, tab };
  }
  return null;
}

export async function closeWorksheetTab(
  worksheetId: string,
  nodeId?: string,
  skipDirtyPrompt = false,
): Promise<boolean> {
  const layoutStore = useLayoutStore.getState();
  const located = nodeId
    ? { nodeId, tab: layoutStore.nodes[nodeId]?.data?.tabs?.find((tab) => tab.id === worksheetId) }
    : findTab(worksheetId);
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

  layoutStore.removeTab(located.nodeId, worksheetId);
  return true;
}

export async function closeEditorTab(
  tabId: string,
  nodeId?: string,
  skipDirtyPrompt = false,
): Promise<boolean> {
  const located = nodeId
    ? {
        nodeId,
        tab: useLayoutStore.getState().nodes[nodeId]?.data?.tabs?.find((t) => t.id === tabId),
      }
    : findTab(tabId);

  const tabType = located?.tab?.type;
  if (tabType === 'worksheet') {
    return closeWorksheetTab(tabId, nodeId, skipDirtyPrompt);
  }
  if (tabType === 'event' || tabType === 'function' || !tabType) {
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
