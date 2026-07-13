import { useMemo } from 'react';
import { useEditorGroupPlacement } from './useEditorGroupPlacement';

/** Narrow subscription for TabBar — tabs + active id only (per group placement). */
export function useEditorGroupTabStrip(groupId: string) {
  const placement = useEditorGroupPlacement(groupId);

  return useMemo(
    () => ({
      tabs: placement.tabs,
      activeTabId: placement.activeTabId ?? undefined,
    }),
    [placement.tabs, placement.activeTabId],
  );
}
