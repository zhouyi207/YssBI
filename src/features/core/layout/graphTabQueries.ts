import { useEditorTabStore } from './editorTabStore';

export function isGraphOpenInAnyTab(graphPath: string): boolean {
  return useEditorTabStore.getState().isTabOpen(graphPath);
}
