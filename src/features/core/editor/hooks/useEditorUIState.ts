/**
 * 编辑器 UI 状态：contextMenu、sidebarDetailFocus、pendingConnection
 * 依赖 useEditorStore
 */

import { useMemo } from 'react';
import { useEditorStore } from '../stores';

export function useEditorUIState() {
  const contextMenu = useEditorStore((s) => s.contextMenu);
  const sidebarDetailFocus = useEditorStore((s) => s.sidebarDetailFocus);
  const pendingConnection = useEditorStore((s) => s.pendingConnection);

  return useMemo(
    () => ({
      contextMenu,
      sidebarDetailFocus,
      pendingConnection,
    }),
    [contextMenu, sidebarDetailFocus, pendingConnection],
  );
}
