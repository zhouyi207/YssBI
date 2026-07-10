import {
  isPending,
  markPending,
  resolvePending,
} from '@/features/core/sync/utils/echoSuppressor';

/** invoke 全量灌图期间抑制 `FunctionUpdated` / `EventUpdated` / `NodePinsUpdated` 自回声。 */
export const GRAPH_REFRESH_ECHO_DOMAIN = 'graphRefreshEcho';

export function markGraphRefreshEcho(graphPaths: readonly string[]): void {
  for (const graphPath of graphPaths) {
    markPending(GRAPH_REFRESH_ECHO_DOMAIN, graphPath);
  }
}

export function resolveGraphRefreshEcho(graphPaths: readonly string[]): void {
  for (const graphPath of graphPaths) {
    resolvePending(GRAPH_REFRESH_ECHO_DOMAIN, graphPath);
  }
}

export function shouldSuppressGraphRefreshEcho(graphPath: string): boolean {
  return isPending(GRAPH_REFRESH_ECHO_DOMAIN, graphPath);
}
