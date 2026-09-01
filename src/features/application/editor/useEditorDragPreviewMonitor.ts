/** Dockview owns editor drag previews; Canvas/sidebar DnD needs no global preview monitor. */
export function clearEditorDragSession(): void {}
export function EditorDragPreviewMonitorHost(): null {
  return null;
}
export function useEditorDragPreviewMonitor(): void {}
