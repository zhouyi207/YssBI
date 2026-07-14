import { WORKBENCH_ROOT_ATTR } from './workbenchDragTypes';

const WORKBENCH_DRAG_IMAGE_CLASS = 'workbench-drag-image';

function resolveDragImageRoot(container: HTMLElement): HTMLElement {
  let node: HTMLElement | null = container;
  while (node && !node.hasAttribute(WORKBENCH_ROOT_ATTR)) {
    node = node.parentElement;
  }
  return node ?? container.ownerDocument.body;
}

/**
 * VS Code `applyDragImage` — ephemeral pill snapshot, removed after `setDragImage`.
 */
export function applyWorkbenchDragImage(
  event: DragEvent,
  container: HTMLElement,
  label: string,
): void {
  if (!event.dataTransfer) return;

  const dragImage = document.createElement('div');
  dragImage.className = WORKBENCH_DRAG_IMAGE_CLASS;
  dragImage.textContent = label;

  const root = resolveDragImageRoot(container);
  root.appendChild(dragImage);
  event.dataTransfer.setDragImage(dragImage, -10, -10);
  setTimeout(() => dragImage.remove(), 0);
}
