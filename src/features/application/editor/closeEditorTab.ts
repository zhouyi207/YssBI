import i18n from 'i18next';

import { editorDockviewPort } from '@/features/core/dockview';
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
import { showBlockingIpcError, showBlockingMessage } from './blockingErrorDialog';

export async function closeWorksheetTab(
  worksheetPath: string,
  nodeId?: string,
  skipDirtyPrompt = false,
): Promise<boolean> {
  const panel = editorDockviewPort
    .findPanelsByResource(worksheetPath)
    .find((candidate) => !nodeId || candidate.groupId === nodeId);
  if (!panel || panel.tab?.kind !== 'worksheet') return false;

  if (isResourceDocumentDirty({ id: worksheetPath, kind: 'worksheet' }) && !skipDirtyPrompt) {
    const displayName = resolveTabDisplayName({ id: worksheetPath, kind: 'worksheet' }, worksheetPath);
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
        if (!context.isCurrent()) return false;
        if (!saved) {
          showBlockingMessage(i18n.t('notifications.editor.worksheetSaveFailed', {
            error: 'worksheet_save_not_committed',
          }));
          return false;
        }
      } catch (error) {
        if (!context.isCurrent()) return false;
        showBlockingIpcError(error, 'save_worksheet', (code) =>
          i18n.t('notifications.editor.worksheetSaveFailed', { error: code }));
        return false;
      }
    }
  }

  await editorDockviewPort.remove(panel.panelInstanceId);
  clearDetailFocusForClosedTab(worksheetPath);
  return true;
}

export async function closeEditorTab(
  tabId: string,
  nodeId?: string,
  skipDirtyPrompt = false,
): Promise<boolean> {
  const panel = editorDockviewPort
    .findPanelsByResource(tabId)
    .find((candidate) => !nodeId || candidate.groupId === nodeId);
  const tabType = panel?.tab?.kind;
  if (tabType === 'worksheet') return closeWorksheetTab(tabId, nodeId, skipDirtyPrompt);
  if (tabType === 'event' || tabType === 'function') return closeGraphTab(tabId, nodeId, skipDirtyPrompt);
  if (!panel) return false;
  return editorDockviewPort.remove(panel.panelInstanceId);
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
    showBlockingIpcError(error, 'remove_worksheet', (code) =>
      i18n.t('notifications.editor.worksheetDeleteFailed', { error: code }));
    return false;
  }
}
