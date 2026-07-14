import type { WorkbenchDragPayload } from './workbenchDragTypes';

/**
 * VS Code `LocalSelectionTransfer` — in-memory drag session for workbench HTML5 DnD.
 * Drop targets read this instead of relying on `dataTransfer` alone.
 */
class WorkbenchDragTransfer {
  private payload: WorkbenchDragPayload | null = null;

  hasData(): boolean {
    return this.payload !== null;
  }

  getData(): WorkbenchDragPayload | null {
    return this.payload;
  }

  setData(payload: WorkbenchDragPayload): void {
    this.payload = payload;
  }

  clearData(): void {
    this.payload = null;
  }
}

export const workbenchDragTransfer = new WorkbenchDragTransfer();
