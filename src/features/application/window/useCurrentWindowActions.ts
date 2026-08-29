import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { toErrorReference, type ErrorReference } from '@/features/application/errorReference';
import { currentAppWindow } from '@/services/platform/appWindow';
import type { PlatformFailure, PlatformOutcome } from '@/services/platform/platformTypes';

export type WindowActionOutcome =
  | { readonly status: 'completed' }
  | { readonly status: 'stale' }
  | { readonly status: 'failed' };

export interface CurrentWindowActions {
  readonly maximized: boolean;
  readonly issue: ErrorReference | null;
  readonly show: () => Promise<WindowActionOutcome>;
  readonly setTitle: (title: string) => Promise<WindowActionOutcome>;
  readonly minimize: () => Promise<WindowActionOutcome>;
  readonly toggleMaximize: () => Promise<WindowActionOutcome>;
  readonly close: () => Promise<WindowActionOutcome>;
}

const WINDOW_ACTION_ERROR_CODE = 'window_action_failed';

export function useCurrentWindowActions(): CurrentWindowActions {
  const window = useMemo(() => currentAppWindow(), []);
  const [maximized, setMaximized] = useState(false);
  const [issue, setIssue] = useState<ErrorReference | null>(null);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    let cleanup: (() => void) | null = null;

    const refresh = async (): Promise<void> => {
      const outcome = await window.isMaximized();
      if (!mounted.current) return;
      if (!outcome.ok) {
        setIssue(platformIssue(outcome.failure));
        return;
      }
      setMaximized(outcome.value);
    };

    const setup = async (): Promise<void> => {
      try {
        await refresh();
        if (!mounted.current) return;

        const subscription = await window.onResized(() => {
          if (!mounted.current) return;
          void refresh().catch((error: unknown) => {
            if (mounted.current) setIssue(toErrorReference(error, WINDOW_ACTION_ERROR_CODE));
          });
        });
        if (!mounted.current) {
          if (subscription.ok) subscription.value();
          return;
        }
        if (subscription.ok) {
          cleanup = subscription.value;
        } else {
          setIssue(platformIssue(subscription.failure));
        }
      } catch (error) {
        if (mounted.current) setIssue(toErrorReference(error, WINDOW_ACTION_ERROR_CODE));
      }
    };

    void setup();
    return () => {
      mounted.current = false;
      cleanup?.();
      cleanup = null;
    };
  }, [window]);

  const run = useCallback(async (
    operation: () => Promise<PlatformOutcome<void>>,
  ): Promise<WindowActionOutcome> => {
    if (!mounted.current) return { status: 'stale' };
    try {
      const outcome = await operation();
      if (!mounted.current) return { status: 'stale' };
      if (!outcome.ok) {
        setIssue(platformIssue(outcome.failure));
        return { status: 'failed' };
      }
      setIssue(null);
      return { status: 'completed' };
    } catch (error) {
      if (!mounted.current) return { status: 'stale' };
      setIssue(toErrorReference(error, WINDOW_ACTION_ERROR_CODE));
      return { status: 'failed' };
    }
  }, []);

  const show = useCallback(() => run(window.show), [run, window]);
  const setTitle = useCallback((title: string) => run(() => window.setTitle(title)), [run, window]);
  const minimize = useCallback(() => run(window.minimize), [run, window]);
  const close = useCallback(() => run(window.close), [run, window]);

  const toggleMaximize = useCallback(async (): Promise<WindowActionOutcome> => {
    const outcome = await run(window.toggleMaximize);
    if (outcome.status !== 'completed' || !mounted.current) return outcome;
    try {
      const refreshed = await window.isMaximized();
      if (!mounted.current) return { status: 'stale' };
      if (!refreshed.ok) {
        setIssue(platformIssue(refreshed.failure));
        return { status: 'failed' };
      }
      setMaximized(refreshed.value);
      return outcome;
    } catch (error) {
      if (!mounted.current) return { status: 'stale' };
      setIssue(toErrorReference(error, WINDOW_ACTION_ERROR_CODE));
      return { status: 'failed' };
    }
  }, [run, window]);

  return useMemo(() => ({
    maximized,
    issue,
    show,
    setTitle,
    minimize,
    toggleMaximize,
    close,
  }), [close, issue, maximized, minimize, setTitle, show, toggleMaximize]);
}

function platformIssue(failure: PlatformFailure): ErrorReference {
  return {
    code: WINDOW_ACTION_ERROR_CODE,
    incidentId: failure.incidentId ?? null,
  };
}
