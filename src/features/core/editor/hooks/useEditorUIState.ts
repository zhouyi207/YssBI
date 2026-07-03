/**
 * 编辑器 UI 状态：contextMenu、detailFocus、pendingConnection
 * 依赖 useEditorStore
 */

import { useMemo } from 'react';
import { useEditorStore } from '../stores';

export function useEditorUIState() {
  const contextMenu = useEditorStore((s) => s.contextMenu);
  const detailFocus = useEditorStore((s) => s.detailFocus);
  const pendingConnection = useEditorStore((s) => s.pendingConnection);

  return useMemo(
    () => ({
      contextMenu,
      detailFocus,
      pendingConnection,
    }),
    [contextMenu, detailFocus, pendingConnection],
  );
}
