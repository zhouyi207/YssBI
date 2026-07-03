import { useMemo } from 'react';
import { useLogStore } from '@/features/core/log/logStore';
import { useEditorStore } from '../stores/useEditorStore';
import { resolveDetailTarget } from './resolveDetailTarget';
import type { DetailTarget } from './types';

export function useDetailTarget(_overrideGroupId?: string | null): DetailTarget | null {
  const detailFocus = useEditorStore((s) => s.detailFocus);
  const selectedLog = useLogStore((s) => s.selectedLog);

  return useMemo(
    () =>
      resolveDetailTarget({
        detailFocus,
        selectedLog,
      }),
    [detailFocus, selectedLog],
  );
}
