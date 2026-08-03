import { portAddressKey } from '@/features/domain/editorProjection';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import type { GraphExecutionState, PinResultState } from '@/shared/types/ui';

export function pinPreviewCacheKey(graphPath: string, port: PortAddressDto): string {
  return `${graphPath.length}:${graphPath}:${portAddressKey(port)}`;
}

export function lookupPinPreview<T>(
  previews: ReadonlyMap<string, T> | undefined,
  graphPath: string,
  port: PortAddressDto,
): T | undefined {
  return previews?.get(pinPreviewCacheKey(graphPath, port));
}

/** Aligns frontend cache keys with backend `(graphPath, pinId)` runtime index. */
export function pinResultCacheKey(graphPath: string, pinId: string): string {
  return `${graphPath}:${pinId}`;
}

export function lookupPinResult(
  pinResults: ReadonlyMap<string, PinResultState> | undefined,
  graphPath: string,
  pinId: string,
): PinResultState | undefined {
  if (!pinResults) return undefined;

  const direct = pinResults.get(pinResultCacheKey(graphPath, pinId));
  if (direct) return direct;

  let fallback: PinResultState | undefined;
  for (const pinResult of pinResults.values()) {
    if (pinResult.pinId !== pinId) continue;
    if (fallback) return undefined;
    fallback = pinResult;
  }
  return fallback;
}

/** Merge pin results emitted during event runs that belong to `sourceGraphPath`. */
export function pinResultsForSourceGraph(
  graphs: Record<string, GraphExecutionState>,
  sourceGraphPath: string,
): Map<string, PinResultState> {
  const merged = new Map<string, PinResultState>();
  for (const bucket of Object.values(graphs)) {
    for (const pinResult of bucket.pinResults.values()) {
      if (pinResult.graphPath === sourceGraphPath) {
        merged.set(pinResultCacheKey(pinResult.graphPath, pinResult.pinId), pinResult);
      }
    }
  }
  return merged;
}

export function executionStatusForSourceGraph(
  graphs: Record<string, GraphExecutionState>,
  sourceGraphPath: string,
): GraphExecutionState['status'] | undefined {
  for (const bucket of Object.values(graphs)) {
    const hasResults = [...bucket.pinResults.values()].some(
      (pinResult) => pinResult.graphPath === sourceGraphPath,
    );
    if (hasResults && bucket.status !== 'idle') {
      return bucket.status;
    }
  }
  return undefined;
}
