import { useWorkbenchStore } from '@/features/core/workbench';

/** Respect an explicit user hide; Dockview/Gridview applies effective visibility. */
export function ensureDetailVisible(): void {
  if (!useWorkbenchStore.getState().detailUserHidden) return;
}
