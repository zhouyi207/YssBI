import { mimeForWorkbenchDragPayload, type WorkbenchDragPayload } from './workbenchDragTypes';
import { workbenchDragTransfer } from './workbenchDragTransfer';

export type FillWorkbenchDragTransferOptions = {
  /**
   * VS Code `disableStandardTransfer` — omit text/uri-list so OS does not treat drag as file export.
   */
  disableStandardTransfer?: boolean;
};

/**
 * VS Code `fillEditorsDragData` subset for workbench detach / auxiliary window drags.
 */
export function fillWorkbenchDragTransfer(
  event: DragEvent,
  payload: WorkbenchDragPayload,
  options?: FillWorkbenchDragTransferOptions,
): void {
  workbenchDragTransfer.setData(payload);
  if (!event.dataTransfer) return;

  event.dataTransfer.setData(mimeForWorkbenchDragPayload(payload), '');
  event.dataTransfer.effectAllowed = 'move';

  if (options?.disableStandardTransfer) {
    return;
  }
}
