import { useMemo } from 'react';
import { useEditorStore } from '../stores';

export function useEditorUIState() {
  const contextMenu = useEditorStore((state) => state.contextMenu);
  const detailFocus = useEditorStore((state) => state.detailFocus);
  return useMemo(() => ({ contextMenu, detailFocus }), [contextMenu, detailFocus]);
}
