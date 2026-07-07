import {
  isPending,
  markPending,
  resolvePending,
} from '@/features/core/sync/utils/echoSuppressor';

/** 全量图刷新期间抑制增量 `NodePinsUpdated`，避免与 `addGraphFromData` 重复应用。 */
export const INCREMENTAL_PIN_UPDATE_GUARD_DOMAIN = 'incrementalPinUpdateGuard';

export function guardFullGraphPinRefresh(graphIds: readonly string[]): () => void {
  for (const id of graphIds) {
    markPending(INCREMENTAL_PIN_UPDATE_GUARD_DOMAIN, id);
  }
  return () => {
    for (const id of graphIds) {
      resolvePending(INCREMENTAL_PIN_UPDATE_GUARD_DOMAIN, id);
    }
  };
}

export function shouldSuppressIncrementalPinUpdate(graphId: string): boolean {
  return isPending(INCREMENTAL_PIN_UPDATE_GUARD_DOMAIN, graphId);
}
