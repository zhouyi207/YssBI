import { useCallback, useEffect, useState } from 'react';
import { logBuffer } from '@/features/core/log/logBuffer';
import { LogService, type DiagnosticSubscription } from '@/services/log';

export type DiagnosticSubscriptionStatus = 'connecting' | 'live' | 'error';

export function useDiagnosticSubscription() {
  const [generation, setGeneration] = useState(0);
  const [status, setStatus] = useState<DiagnosticSubscriptionStatus>('connecting');

  useEffect(() => {
    let cancelled = false;
    let subscription: DiagnosticSubscription | null = null;
    setStatus('connecting');

    void LogService.subscribeDiagnostics((batch) => {
      if (!cancelled) logBuffer.appendBatch(batch);
    }).then((nextSubscription) => {
      if (cancelled) {
        void nextSubscription.unsubscribe().catch(() => {});
        return;
      }
      subscription = nextSubscription;
      logBuffer.setSubscription(nextSubscription.snapshot);
      nextSubscription.activate();
      setStatus('live');
    }).catch((error) => {
      if (cancelled) return;
      console.error('[Diagnostics] Failed to subscribe', error);
      setStatus('error');
    });

    return () => {
      cancelled = true;
      if (subscription) void subscription.unsubscribe().catch(() => {});
    };
  }, [generation]);

  const reconnect = useCallback(() => {
    setGeneration((current) => current + 1);
  }, []);

  return { status, reconnect };
}
