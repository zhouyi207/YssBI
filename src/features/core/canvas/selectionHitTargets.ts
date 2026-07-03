export interface SelectionHitTarget {
  id: string;
  left: number;
  right: number;
  top: number;
  bottom: number;
}

export function queryCanvasElement(groupId: string): HTMLElement | null {
  return document.querySelector(`[data-editor-group-id="${groupId}"]`);
}

export function buildSelectionHitTargets(
  canvasEl: HTMLElement,
  nodeIds: readonly string[],
): SelectionHitTarget[] {
  const targets: SelectionHitTarget[] = [];

  for (const nodeId of nodeIds) {
    const element = canvasEl.querySelector(`[data-node-id="${nodeId}"]`);
    if (!element) continue;

    const bounds = element.getBoundingClientRect();
    targets.push({
      id: nodeId,
      left: bounds.left,
      right: bounds.right,
      top: bounds.top,
      bottom: bounds.bottom,
    });
  }

  return targets;
}

export function hitTestSelection(
  targets: readonly SelectionHitTarget[],
  rect: { x1: number; y1: number; x2: number; y2: number },
): string[] {
  const selectedIds: string[] = [];
  for (const target of targets) {
    if (target.left > rect.x2 || target.right < rect.x1 || target.top > rect.y2 || target.bottom < rect.y1) {
      continue;
    }
    selectedIds.push(target.id);
  }
  return selectedIds;
}

export function syncSelectionPreview(
  canvasEl: HTMLElement,
  previousIds: readonly string[],
  nextIds: readonly string[],
): void {
  const nextSet = new Set(nextIds);
  for (const id of previousIds) {
    if (!nextSet.has(id)) {
      canvasEl.querySelector(`[data-node-id="${id}"]`)?.removeAttribute('data-selection-preview');
    }
  }

  const previousSet = new Set(previousIds);
  for (const id of nextIds) {
    if (!previousSet.has(id)) {
      canvasEl.querySelector(`[data-node-id="${id}"]`)?.setAttribute('data-selection-preview', 'true');
    }
  }
}

export function clearAllSelectionPreview(canvasEl: HTMLElement): void {
  canvasEl.querySelectorAll('[data-selection-preview="true"]').forEach((el) => {
    el.removeAttribute('data-selection-preview');
  });
}
