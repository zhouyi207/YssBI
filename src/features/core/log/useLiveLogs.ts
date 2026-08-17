/**
 * Bridges backend diagnostic batches from the external recent buffer into React.
 * Each accepted Channel batch publishes one stable snapshot.
 */
import { useSyncExternalStore } from 'react';
import { logBuffer, type LogSnapshot } from './logBuffer';

export function useLiveLogs(): LogSnapshot {
  return useSyncExternalStore(logBuffer.subscribe, logBuffer.getSnapshot);
}
