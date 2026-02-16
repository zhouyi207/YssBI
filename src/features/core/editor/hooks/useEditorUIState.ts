/**
 * 编辑器 UI 状态：contextMenu、selectedItemId、pendingConnection
 * 依赖 useEditorStore
 */

import { useMemo } from 'react';
import { useEditorStore } from '../stores';

export function useEditorUIState() {
  const contextMenu = useEditorStore((s) => s.contextMenu);
  const selectedItemId = useEditorStore((s) => s.selectedItemId);
  const selectedItemType = useEditorStore((s) => s.selectedItemType);
  const pendingConnection = useEditorStore((s) => s.pendingConnection);

  return useMemo(
    () => ({
      contextMenu,
      selectedItemId,
      selectedItemType,
      pendingConnection,
    }),
    [contextMenu, selectedItemId, selectedItemType, pendingConnection]
  );
}
