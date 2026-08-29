import { resultRef, type InspectableResultRef } from '@/features/domain/result/inspectableResultRef';
import type { PinHistoryProjection } from '@/shared/types/ui';
import { pinHistoryCacheKey } from './pinResultIndex';

export interface PinResultSearchEntry {
  id: string;
  ref: InspectableResultRef;
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
  history: PinHistoryProjection,
  labels: PinResultSearchLabels,
): PinResultSearchEntry | null {
  const selected = history.entries.find((entry) => entry.resultId === history.selectedResultId)
    ?? history.entries[history.entries.length - 1];
  if (!selected) return null;

  const nodeTitle = labels.nodeTitle.trim();
  const pinName = labels.pinName.trim();
  const sourceTitle = `${selected.state.kind} · ${selected.resultId}`;
  const searchText = [nodeTitle, pinName, sourceTitle, history.graphPath, selected.runId]
    .join(' ')
    .toLowerCase();

  return {
    id: pinHistoryCacheKey(history.graphPath, history.output),
    ref: resultRef(selected.resultId),
    nodeTitle,
    pinName,
    sourceTitle,
    searchText,
  };
}

export function collectPinResultSearchEntries(
  histories: ReadonlyMap<string, PinHistoryProjection>,
  resolveLabels: (history: PinHistoryProjection) => PinResultSearchLabels,
): PinResultSearchEntry[] {
  return [...histories.values()]
    .flatMap((history) => {
      const entry = buildPinResultSearchEntry(history, resolveLabels(history));
      return entry ? [entry] : [];
    })
    .sort((left, right) => left.searchText.localeCompare(right.searchText));
}

export function filterPinResultSearchEntries(
  entries: PinResultSearchEntry[],
  query: string,
): PinResultSearchEntry[] {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return entries;
  return entries.filter((entry) => entry.searchText.includes(normalized));
}
