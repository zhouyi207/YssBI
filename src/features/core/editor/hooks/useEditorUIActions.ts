/**
 * 编辑器 UI 操作：setContextMenu、setSelectedInfo、setPendingConnection
 */
import { useEditorStore } from '../stores';

export function useEditorUIActions() {
  const setContextMenu = useEditorStore((s) => s.setContextMenu);
  const setSelectedInfo = useEditorStore((s) => s.setSelectedInfo);
  const setPendingConnection = useEditorStore((s) => s.setPendingConnection);

  return { setContextMenu, setSelectedInfo, setPendingConnection };
}
