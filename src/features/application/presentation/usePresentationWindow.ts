import { useEffect, useMemo, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { WindowKind } from '@/shared/types/settings';
import { usePersistedWindow } from '@/features/application/window/usePersistedWindow';
import { usePresentationWindowLifecycle } from '@/features/application/window/usePresentationWindowLifecycle';
import { useWindowMaximized } from '@/features/application/window/useWindowMaximized';
import { logger } from '@/utils/appLogger';
import { loadPresentationWindow, type PresentationWindowState } from './loadPresentationWindow';
import { parseSourceIdFromLocation } from './parseSourceIdFromLocation';

export function usePresentationWindow(windowKind: WindowKind, logTag: string) {
  const sourceId = useMemo(() => parseSourceIdFromLocation(), []);
  const [state, setState] = useState<PresentationWindowState>(() =>
    sourceId ? { status: 'loading' } : { status: 'missing_source_id' },
  );

  usePresentationWindowLifecycle(sourceId);
  usePersistedWindow(windowKind);
  const isMaximized = useWindowMaximized(logTag);

  useEffect(() => {
    let cancelled = false;

    const revealWindow = async (title?: string) => {
      if (title) {
        await getCurrentWindow().setTitle(title).catch(() => {});
      }
      await getCurrentWindow().show().catch((error) => {
        logger.app.error(
          `Failed to show window: ${error instanceof Error ? error.message : String(error)}`,
          logTag,
        );
      });
    };

    if (!sourceId) {
      void revealWindow();
      return;
    }

    void (async () => {
      const next = await loadPresentationWindow(sourceId);
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
  }, [sourceId, logTag]);

  return { sourceId, state, isMaximized };
}
