import { useCallback, useMemo } from 'react';
import { currentAppWindow } from '@/services/platform/appWindow';
import { logger } from '@/utils/appLogger';

export interface CurrentWindowActions {
  readonly show: () => Promise<void>;
  readonly minimize: () => Promise<void>;
  readonly maximize: () => Promise<void>;
  readonly close: () => Promise<void>;
  readonly setTitle: (title: string) => Promise<void>;
}

export function useCurrentWindowActions(logTag = 'Window'): CurrentWindowActions {
  const window = useMemo(() => currentAppWindow(), []);

  const run = useCallback(async (operation: () => Promise<{ ok: true } | { ok: false }>) => {
    const result = await operation();
    if (!result.ok) logger.sys.warn('window operation failed', logTag);
  }, [logTag]);

  const show = useCallback(() => run(window.show), [run, window]);
  const minimize = useCallback(() => run(window.minimize), [run, window]);
  const maximize = useCallback(() => run(window.toggleMaximize), [run, window]);
  const close = useCallback(() => run(window.close), [run, window]);
  const setTitle = useCallback((title: string) => run(() => window.setTitle(title)), [run, window]);

  return useMemo(() => ({ show, minimize, maximize, close, setTitle }), [show, minimize, maximize, close, setTitle]);
}
