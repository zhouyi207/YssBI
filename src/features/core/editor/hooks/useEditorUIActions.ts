import { useEditorStore } from '../stores';

export function useEditorUIActions() {
  const setContextMenu = useEditorStore((state) => state.setContextMenu);
  const setDetailFocus = useEditorStore((state) => state.setDetailFocus);
  const clearDetailFocus = useEditorStore((state) => state.clearDetailFocus);
  return { setContextMenu, setDetailFocus, clearDetailFocus };
}
