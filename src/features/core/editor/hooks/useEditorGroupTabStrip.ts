import { useShallow } from 'zustand/react/shallow';
import type { LayoutTab } from '@/shared/types';
import { useLayoutStore } from '@/features/core/layout/layoutStore';

const EMPTY_TABS: LayoutTab[] = [];

/** Narrow layout subscription for TabBar — tabs + active id only. */
export function useEditorGroupTabStrip(groupId: string) {
  return useLayoutStore(
    useShallow((state) => {
      const data = state.nodes[groupId]?.data;
      // Return store references as-is — never map/normalize here (unstable snapshot → infinite loop).
      return {
        tabs: data?.tabs ?? EMPTY_TABS,
        activeTabId: data?.activeTabId,
      };
    }),
  );
}
