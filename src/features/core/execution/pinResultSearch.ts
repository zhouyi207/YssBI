import type { InspectableSourceRef } from '@/features/core/resultSource/inspectableSource';
import { runtimePinRef } from '@/features/core/resultSource/inspectableSource';
import type { PinResultState } from '@/shared/types/ui';
import { pinResultCacheKey } from './pinResultIndex';

export interface PinResultSearchEntry {
  id: string;
  ref: InspectableSourceRef;
  nodeTitle: string;
  pinName: string;
  sourceTitle: string;
  searchText: string;
}

export interface PinResultSearchLabels {
  nodeTitle: string;
  pinName: string;
}

export function buildPinResultSearchEntry(
  pinResult: PinResultState,
  labels: PinResultSearchLabels,
): PinResultSearchEntry {
  const sourceTitle = pinResult.descriptor.title.trim();
  const nodeTitle = labels.nodeTitle.trim() || pinResult.nodeId || pinResult.pinId;
  const pinName = labels.pinName.trim() || pinResult.pinId;
  const searchText = [nodeTitle, pinName, sourceTitle, pinResult.graphPath]
    .join(' ')
    .toLowerCase();

  return {
    id: pinResultCacheKey(pinResult.graphPath, pinResult.pinId),
    ref: runtimePinRef(pinResult.graphPath, pinResult.pinId),
    nodeTitle,
    pinName,
    sourceTitle,
    searchText,
  };
}

/** Build searchable entries from execution pinResults — the post-run source of truth. */
export function collectPinResultSearchEntries(
  pinResults: ReadonlyMap<string, PinResultState>,
  resolveLabels: (
    labelGraphPath: string,
    nodeId: string,
    pinId: string,
  ) => PinResultSearchLabels,
): PinResultSearchEntry[] {
  const entries: PinResultSearchEntry[] = [];

  for (const pinResult of pinResults.values()) {
    entries.push(
      buildPinResultSearchEntry(
        pinResult,
        resolveLabels(pinResult.graphPath, pinResult.nodeId, pinResult.pinId),
      ),
    );
  }

  return entries.sort((left, right) => left.searchText.localeCompare(right.searchText));
}

export function filterPinResultSearchEntries(
  entries: PinResultSearchEntry[],
  query: string,
): PinResultSearchEntry[] {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return entries;
  return entries.filter((entry) => entry.searchText.includes(normalized));
}
