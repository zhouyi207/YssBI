import type { ExecutionVisualSnapshot } from './executionVisualSession';

const EXEC_STATE_ATTR = 'data-exec-state';

export function clearExecutionVisualDom(canvas: HTMLElement): void {
  canvas.querySelectorAll(`[${EXEC_STATE_ATTR}]`).forEach((el) => {
    el.removeAttribute(EXEC_STATE_ATTR);
  });
}

export function syncExecutionVisualDom(canvas: HTMLElement, snap: ExecutionVisualSnapshot): void {
  if (!snap.active) {
    clearExecutionVisualDom(canvas);
    return;
  }

  canvas.querySelectorAll(`[${EXEC_STATE_ATTR}]`).forEach((el) => {
    el.removeAttribute(EXEC_STATE_ATTR);
  });

  if (snap.executingNodeId) {
    canvas
      .querySelector(`[data-node-id="${snap.executingNodeId}"]`)
      ?.setAttribute(EXEC_STATE_ATTR, 'executing');
  }

  for (const nodeId of snap.executedNodeIds) {
    if (nodeId === snap.executingNodeId) continue;
    canvas.querySelector(`[data-node-id="${nodeId}"]`)?.setAttribute(EXEC_STATE_ATTR, 'completed');
  }

  for (const nodeId of snap.errorNodeIds) {
    canvas.querySelector(`[data-node-id="${nodeId}"]`)?.setAttribute(EXEC_STATE_ATTR, 'error');
  }
}
