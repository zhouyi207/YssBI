/**
 * VS Code `editorTabsControl.isNewWindowOperation`:
 * when `dragToOpenWindow` is enabled, Alt inverts; when disabled, Alt enables.
 */
export function isDragToOpenWindowOperation(
  event: Pick<DragEvent, 'altKey'>,
  dragToOpenWindowEnabled: boolean,
): boolean {
  if (dragToOpenWindowEnabled) return !event.altKey;
  return event.altKey;
}
