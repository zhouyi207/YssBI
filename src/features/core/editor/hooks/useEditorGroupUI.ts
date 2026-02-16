import { useMemo } from 'react';
import { useEditorGroup } from '@/features/application/editor/core/hooks/useEditorGroup';

export function useEditorGroupUI() {
  const {
    contextMenu,
    pendingConnection,
    setContextMenu,
    setPendingConnection,
    setSelectedInfo,
  } = useEditorGroup();

  return useMemo(() => ({
    contextMenu,
    pendingConnection,
    setContextMenu,
    setPendingConnection,
    setSelectedInfo,
  }), [
    contextMenu,
    pendingConnection,
    setContextMenu,
    setPendingConnection,
    setSelectedInfo,
  ]);
}
