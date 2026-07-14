import { useEditorDropPreviewStore } from './editorDropPreviewStore';
import {
  findEditorGroupAtPointer,
  findTabBarTargetFromPointer,
} from '@/features/core/layout/editorDropTarget';
import { isSidebarItemDropAllowedAtPointer } from '@/features/core/layout/workbenchSidebarDropSurface';
import { useSidebarDragStore } from '@/features/core/sidebarDrag';

/** VS Code `onEditorAreaLeave` — clear split overlay after pointer leaves drop targets. */
const DROP_PREVIEW_STALE_MS = 300;

let staleTimer: ReturnType<typeof setTimeout> | null = null;

function hasEditorDropTargetAt(clientX: number, clientY: number): boolean {
  if (useSidebarDragStore.getState().activeDrag) {
    return isSidebarItemDropAllowedAtPointer(clientX, clientY);
  }

  return (
    findTabBarTargetFromPointer(clientX, clientY) !== null
    || findEditorGroupAtPointer(clientX, clientY) !== null
  );
}

export function cancelDropPreviewStaleGuard(): void {
  if (!staleTimer) return;
  clearTimeout(staleTimer);
  staleTimer = null;
}

export function refreshDropPreviewStaleGuard(clientX: number, clientY: number): void {
  if (hasEditorDropTargetAt(clientX, clientY)) {
    cancelDropPreviewStaleGuard();
    return;
  }

  if (staleTimer) return;

  staleTimer = setTimeout(() => {
    staleTimer = null;
    useEditorDropPreviewStore.getState().clearPreview();
  }, DROP_PREVIEW_STALE_MS);
}
