import {
  isPending,
  markPending,
  resolvePending,
} from '@/features/core/sync/utils/echoSuppressor';

/** 全量图刷新期间抑制增量 `NodePinsUpdated`，避免与 `addGraphFromData` 重复应用。 */
export const INCREMENTAL_PIN_UPDATE_GUARD_DOMAIN = 'incrementalPinUpdateGuard';

export function guardFullGraphPinRefresh(graphPaths: readonly string[]): () => void {
  for (const graphPath of graphPaths) {
    markPending(INCREMENTAL_PIN_UPDATE_GUARD_DOMAIN, graphPath);
  }
  return () => {
    for (const graphPath of graphPaths) {
      resolvePending(INCREMENTAL_PIN_UPDATE_GUARD_DOMAIN, graphPath);
    }
  };
}

export function shouldSuppressIncrementalPinUpdate(graphPath: string): boolean {
  return isPending(INCREMENTAL_PIN_UPDATE_GUARD_DOMAIN, graphPath);
}
