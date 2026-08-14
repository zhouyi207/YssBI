import { isSidebarSpawnDrag } from '@/features/core/dnd';

function findEditorCanvasAtPointer(clientX: number, clientY: number): HTMLElement | null {
  for (const element of document.elementsFromPoint(clientX, clientY)) {
    if (!(element instanceof HTMLElement)) continue;
    const canvas = element.closest<HTMLElement>('[data-editor-group-id]');
    if (canvas?.dataset.editorGroupId) return canvas;
  }
  return null;
}

/** Sidebar palette items may only drop on a Dockview editor Canvas. */
export function isSidebarSpawnDropAllowed(
  data: unknown,
  pointer: { x: number; y: number } | null,
): boolean {
  return isSidebarSpawnDrag(data)
    && pointer !== null
    && findEditorCanvasAtPointer(pointer.x, pointer.y) !== null;
}

export function isSidebarSpawnDropAllowedAtPointer(
  clientX: number,
  clientY: number,
): boolean {
  return findEditorCanvasAtPointer(clientX, clientY) !== null;
}

export function findSidebarDropCanvasAtPointer(
  clientX: number,
  clientY: number,
): { groupId: string; bounds: DOMRect } | null {
  const canvas = findEditorCanvasAtPointer(clientX, clientY);
  const groupId = canvas?.dataset.editorGroupId;
  return canvas && groupId ? { groupId, bounds: canvas.getBoundingClientRect() } : null;
}
