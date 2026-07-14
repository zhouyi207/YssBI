export type DragSurfaceMode = 'strict' | 'header-row';

function isElementNode(value: unknown): value is Element {
  return typeof value === 'object' && value !== null && 'closest' in value;
}

function isContainerNode(value: unknown): value is Element {
  return typeof value === 'object' && value !== null && 'contains' in value;
}

export function acceptsDragStart(
  event: Pick<DragEvent, 'target' | 'currentTarget'>,
  mode: DragSurfaceMode,
): boolean {
  if (mode === 'strict') {
    return event.target === event.currentTarget;
  }

  const handle = event.currentTarget;
  const target = event.target;
  if (!isElementNode(target) || !isContainerNode(handle)) return false;
  if (!handle.contains(target)) return false;
  if (target.closest('button, a, input, textarea, select, [role="button"]')) return false;
  return true;
}
