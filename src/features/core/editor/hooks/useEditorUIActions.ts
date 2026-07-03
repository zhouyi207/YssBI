/**
 * 编辑器 UI 操作：setContextMenu、sidebarDetailFocus、setPendingConnection
 */
import { useEditorStore } from '../stores';

export function useEditorUIActions() {
  const setContextMenu = useEditorStore((s) => s.setContextMenu);
  const setSidebarDetailFocus = useEditorStore((s) => s.setSidebarDetailFocus);
  const clearSidebarDetailFocus = useEditorStore((s) => s.clearSidebarDetailFocus);
  const setPendingConnection = useEditorStore((s) => s.setPendingConnection);

  return { setContextMenu, setSidebarDetailFocus, clearSidebarDetailFocus, setPendingConnection };
}
