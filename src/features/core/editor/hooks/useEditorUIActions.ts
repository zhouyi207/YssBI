/**
 * 编辑器 UI 操作：setContextMenu、detailFocus、setPendingConnection
 */
import { useEditorStore } from '../stores';

export function useEditorUIActions() {
  const setContextMenu = useEditorStore((s) => s.setContextMenu);
  const setDetailFocus = useEditorStore((s) => s.setDetailFocus);
  const clearDetailFocus = useEditorStore((s) => s.clearDetailFocus);
  const setPendingConnection = useEditorStore((s) => s.setPendingConnection);

  return { setContextMenu, setDetailFocus, clearDetailFocus, setPendingConnection };
}
