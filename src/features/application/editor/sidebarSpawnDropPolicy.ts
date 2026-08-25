import { isSidebarSpawnDrag } from '@/features/core/dnd';

function findEditorCanvasAtPointer(clientX: number, clientY: number): HTMLElement | null {
  for (const element of document.elementsFromPoint(clientX, clientY)) {
    if (!(element instanceof HTMLElement)) continue;
    const canvas = element.closest<HTMLElement>('[data-editor-panel-instance-id]');
    if (canvas?.dataset.editorPanelInstanceId) return canvas;
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
): { panelInstanceId: string; groupId: string; graphPath: string; graphKind: 'event' | 'function'; bounds: DOMRect } | null {
  const canvas = findEditorCanvasAtPointer(clientX, clientY);
  const panelInstanceId = canvas?.dataset.editorPanelInstanceId;
  const groupId = canvas?.dataset.editorGroupId;
  const graphPath = canvas?.dataset.editorGraphPath;
  const graphKind = canvas?.dataset.editorGraphKind;
  return canvas && panelInstanceId && groupId && graphPath
    && (graphKind === 'event' || graphKind === 'function')
    ? { panelInstanceId, groupId, graphPath, graphKind, bounds: canvas.getBoundingClientRect() }
    : null;
}
