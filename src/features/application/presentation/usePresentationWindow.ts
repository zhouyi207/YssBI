import { useEffect, useMemo, useState } from 'react';
import type { WindowKind } from '@/shared/types/settings';
import { usePersistedWindow } from '@/features/application/window/usePersistedWindow';
import { usePresentationWindowLifecycle } from '@/features/application/window/usePresentationWindowLifecycle';
import { useWindowMaximized } from '@/features/application/window/useWindowMaximized';
import { useCurrentWindowActions } from '@/features/application/window/useCurrentWindowActions';
import { loadPresentationWindow, type PresentationWindowState } from './loadPresentationWindow';
import { parsePresentationWindowQuery } from './parsePresentationWindowQuery';

export function usePresentationWindow(windowKind: WindowKind, logTag: string) {
  const query = useMemo(() => parsePresentationWindowQuery(), []);
  const resultId = query.resultId;
  const [state, setState] = useState<PresentationWindowState>(() =>
    resultId ? { status: 'loading' } : { status: 'missing_result_id' },
  );

  usePresentationWindowLifecycle(resultId);
  usePersistedWindow(windowKind);
  const isMaximized = useWindowMaximized(logTag);
  const windowActions = useCurrentWindowActions(logTag);

  useEffect(() => {
    let cancelled = false;

    const revealWindow = async (title?: string) => {
      if (title) await windowActions.setTitle(title);
      await windowActions.show();
    };

    if (!resultId) {
      void revealWindow();
      return;
    }

    void (async () => {
      const next = await loadPresentationWindow(resultId);
      if (cancelled) return;
      setState(next);
      if (next.status === 'ready') {
        await revealWindow(next.descriptor.title);
        return;
      }
      await revealWindow();
    })();

    return () => {
      cancelled = true;
    };
  }, [resultId, windowActions]);

  return { resultId, state, isMaximized };
}
