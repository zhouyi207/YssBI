import { useMemo } from 'react';
import { useLogStore } from '@/features/core/log/logStore';
import { useActiveEditorGroup } from '../hooks/useActiveEditorGroup';
import { useEditorStore } from '../stores/useEditorStore';
import { resolveDetailTarget } from './resolveDetailTarget';
import type { DetailTarget } from './types';

export function useDetailTarget(overrideGroupId?: string | null): DetailTarget | null {
  const { activeTabId, tabs, selectedNodeIds } = useActiveEditorGroup(overrideGroupId);
  const sidebarDetailFocus = useEditorStore((s) => s.sidebarDetailFocus);
  const selectedLog = useLogStore((s) => s.selectedLog);

  return useMemo(
    () =>
      resolveDetailTarget({
        activeTabId,
        tabs,
        selectedNodeIds,
        sidebarDetailFocus,
        selectedLog,
      }),
    [activeTabId, tabs, selectedNodeIds, sidebarDetailFocus, selectedLog],
  );
}
