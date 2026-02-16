/**
 * 获取所有编辑器组
 * 依赖 layout store
 */

import { useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';

export function useEditorGroups() {
  const groupNodes = useLayoutStore(
    useShallow((s: LayoutState) => Object.values(s.nodes).filter((n: any) => n.type === 'component' && n.data?.tabs))
  );

  return useMemo(
    () =>
      groupNodes.map((n: any) => ({
        id: n.id,
        tabs: (n.data?.tabs || []).map((t: any) => ({ ...t, type: t.type || 'event' })) as any[],
        activeTabId: n.data?.activeTabId || null,
        selectedNodeIds: n.data?.params?.selectedNodeIds || [],
      })),
    [groupNodes]
  );
}
