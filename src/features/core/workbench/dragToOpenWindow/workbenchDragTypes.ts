/** HTML5 marker types — cross-window only; in-window state uses `workbenchDragTransfer`. */
export const WORKBENCH_DRAG_MIME = {
  LOGS_PANEL: 'application/x-yssbi-logs-drag',
  EDITOR: 'application/x-yssbi-editor-drag',
} as const;

export type WorkbenchDragMime = (typeof WORKBENCH_DRAG_MIME)[keyof typeof WORKBENCH_DRAG_MIME];

export type WorkbenchDragPayload =
  | { kind: 'logs-panel' }
  | { kind: 'editor' };

export const WORKBENCH_ROOT_ATTR = 'data-yssbi-workbench';

export function mimeForWorkbenchDragPayload(payload: WorkbenchDragPayload): WorkbenchDragMime {
  if (payload.kind === 'logs-panel') return WORKBENCH_DRAG_MIME.LOGS_PANEL;
  return WORKBENCH_DRAG_MIME.EDITOR;
}
