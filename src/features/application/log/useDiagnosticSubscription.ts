import { useCallback, useEffect, useRef, useState } from "react";
import { logBuffer } from "@/features/application/log/logBuffer";
import { LogService, type DiagnosticSubscription } from "@/services/log";

export type DiagnosticSubscriptionStatus = "connecting" | "live" | "error";

export function useDiagnosticSubscription() {
  const recoveryAttempts = useRef(0);
  const [generation, setGeneration] = useState(0);
  const [status, setStatus] = useState<DiagnosticSubscriptionStatus>("connecting");

  useEffect(() => {
    let cancelled = false;
    let discontinued = false;
    let subscription: DiagnosticSubscription | null = null;
    setStatus("connecting");

    void LogService.subscribeDiagnostics(
      (batch) => {
        if (!cancelled && !discontinued) {
          recoveryAttempts.current = 0;
          logBuffer.appendBatch(batch);
        }
      },
      () => {
        if (cancelled || discontinued) return;
        discontinued = true;
        logBuffer.markTruncated();
        if (recoveryAttempts.current < 3) {
          recoveryAttempts.current += 1;
          setStatus("connecting");
          setGeneration((current) => current + 1);
        } else {
          setStatus("error");
        }
      },
    )
      .then((nextSubscription) => {
        if (cancelled) {
          void nextSubscription.unsubscribe().catch(() => {});
          return;
        }
        subscription = nextSubscription;
        logBuffer.setSubscription(nextSubscription.snapshot);
        nextSubscription.activate();
        if (!discontinued) setStatus("live");
      })
      .catch((error) => {
        if (cancelled) return;
        console.error("[Diagnostics] Failed to subscribe", error);
        setStatus("error");
      });

    return () => {
      cancelled = true;
      if (subscription) void subscription.unsubscribe().catch(() => {});
    };
  }, [generation]);

  const reconnect = useCallback(() => {
    recoveryAttempts.current = 0;
    setGeneration((current) => current + 1);
  }, []);

  return { status, reconnect };
}
