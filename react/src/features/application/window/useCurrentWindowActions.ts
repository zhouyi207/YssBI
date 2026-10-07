import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toErrorReference, type ErrorReference } from "@/features/application/errorReference";
import { currentAppWindow } from "@/services/platform/appWindow";
import type { PlatformFailure, PlatformOutcome } from "@/services/platform/platformTypes";

export type WindowActionOutcome =
  | { readonly status: "completed" }
  | { readonly status: "stale" }
  | { readonly status: "failed" };

export interface CurrentWindowActions {
  readonly maximized: boolean;
  readonly issue: ErrorReference | null;
  readonly show: () => Promise<WindowActionOutcome>;
  readonly setTitle: (title: string) => Promise<WindowActionOutcome>;
  readonly minimize: () => Promise<WindowActionOutcome>;
  readonly toggleMaximize: () => Promise<WindowActionOutcome>;
  readonly close: () => Promise<WindowActionOutcome>;
}

const WINDOW_ACTION_ERROR_CODE = "window_action_failed";

export function useCurrentWindowActions(): CurrentWindowActions {
  const window = useMemo(() => currentAppWindow(), []);
  const [maximized, setMaximized] = useState(false);
  const [issue, setIssue] = useState<ErrorReference | null>(null);
  const lifetime = useRef<object | null>(null);

  useEffect(() => {
    const current = {};
    lifetime.current = current;
    const isCurrent = () => lifetime.current === current;
    let cleanup: (() => void) | null = null;

    const refresh = async (): Promise<void> => {
      const outcome = await window.isMaximized();
      if (!isCurrent()) return;
      if (!outcome.ok) {
        setIssue(platformIssue(outcome.failure));
        return;
      }
      setMaximized(outcome.value);
    };

    const setup = async (): Promise<void> => {
      try {
        await refresh();
        if (!isCurrent()) return;

        const subscription = await window.onResized(() => {
          if (!isCurrent()) return;
          void refresh().catch((error: unknown) => {
            if (isCurrent()) setIssue(toErrorReference(error, WINDOW_ACTION_ERROR_CODE));
          });
        });
        if (!isCurrent()) {
          if (subscription.ok) subscription.value();
          return;
        }
        if (subscription.ok) {
          cleanup = subscription.value;
        } else {
          setIssue(platformIssue(subscription.failure));
        }
      } catch (error) {
        if (isCurrent()) setIssue(toErrorReference(error, WINDOW_ACTION_ERROR_CODE));
      }
    };

    void setup();
    return () => {
      lifetime.current = null;
      cleanup?.();
      cleanup = null;
    };
  }, [window]);

  const run = useCallback(
    async (operation: () => Promise<PlatformOutcome<void>>): Promise<WindowActionOutcome> => {
      const current = lifetime.current;
      if (current === null) return { status: "stale" };
      try {
        const outcome = await operation();
        if (lifetime.current !== current) return { status: "stale" };
        if (!outcome.ok) {
          setIssue(platformIssue(outcome.failure));
          return { status: "failed" };
        }
        setIssue(null);
        return { status: "completed" };
      } catch (error) {
        if (lifetime.current !== current) return { status: "stale" };
        setIssue(toErrorReference(error, WINDOW_ACTION_ERROR_CODE));
        return { status: "failed" };
      }
    },
    [],
  );

  const show = useCallback(() => run(window.show), [run, window]);
  const setTitle = useCallback((title: string) => run(() => window.setTitle(title)), [run, window]);
  const minimize = useCallback(() => run(window.minimize), [run, window]);
  const close = useCallback(() => run(window.close), [run, window]);

  const toggleMaximize = useCallback(async (): Promise<WindowActionOutcome> => {
    const current = lifetime.current;
    const outcome = await run(window.toggleMaximize);
    if (outcome.status !== "completed") return outcome;
    if (lifetime.current !== current) return { status: "stale" };
    try {
      const refreshed = await window.isMaximized();
      if (lifetime.current !== current) return { status: "stale" };
      if (!refreshed.ok) {
        setIssue(platformIssue(refreshed.failure));
        return { status: "failed" };
      }
      setMaximized(refreshed.value);
      return outcome;
    } catch (error) {
      if (lifetime.current !== current) return { status: "stale" };
      setIssue(toErrorReference(error, WINDOW_ACTION_ERROR_CODE));
      return { status: "failed" };
    }
  }, [run, window]);

  return useMemo(
    () => ({
      maximized,
      issue,
      show,
      setTitle,
      minimize,
      toggleMaximize,
      close,
    }),
    [close, issue, maximized, minimize, setTitle, show, toggleMaximize],
  );
}

function platformIssue(failure: PlatformFailure): ErrorReference {
  return {
    code: WINDOW_ACTION_ERROR_CODE,
    incidentId: failure.incidentId ?? null,
  };
}
