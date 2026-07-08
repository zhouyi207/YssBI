import type { PinResultState } from '@/shared/types/ui';
import { isInspectableDataPin, resolvePinViewTargetFromCache } from './pinViewTarget';

export type PinResultSearchDirection = 'input' | 'output';

export interface PinResultSearchEntry {
  id: string;
  direction: PinResultSearchDirection;
  pinResult: PinResultState;
  nodeTitle: string;
  pinName: string;
  sourceTitle: string;
  searchText: string;
}

export interface PinResultSearchLabels {
  nodeTitle: string;
  pinName: string;
}

export interface PinResultSearchPinRef {
  pinId: string;
  nodeId: string;
  direction: PinResultSearchDirection;
  isExec: boolean;
  connectionIds: readonly string[];
}

export function buildPinResultSearchEntry(
  id: string,
  direction: PinResultSearchDirection,
  pinResult: PinResultState,
  labels: PinResultSearchLabels,
): PinResultSearchEntry {
  const sourceTitle = pinResult.descriptor.title.trim();
  const nodeTitle = labels.nodeTitle.trim();
  const pinName = labels.pinName.trim();
  const searchText = [nodeTitle, pinName, sourceTitle, direction].join(' ').toLowerCase();

  return {
    id,
    direction,
    pinResult,
    nodeTitle,
    pinName,
    sourceTitle,
    searchText,
  };
}

export function collectPinResultSearchEntries(
  graphId: string,
  pinResults: ReadonlyMap<string, PinResultState>,
  pins: readonly PinResultSearchPinRef[],
  resolveLabels: (nodeId: string, pinId: string) => PinResultSearchLabels,
): PinResultSearchEntry[] {
  const entries: PinResultSearchEntry[] = [];

  for (const pin of pins) {
    if (!isInspectableDataPin(pin.isExec)) continue;

    if (pin.direction === 'output') {
      const pinResult = pinResults.get(pin.pinId);
      if (!pinResult || pinResult.graphId !== graphId) continue;

      entries.push(
        buildPinResultSearchEntry(
          `output:${pin.pinId}`,
          'output',
          pinResult,
          resolveLabels(pin.nodeId, pin.pinId),
        ),
      );
      continue;
    }

    if (pin.connectionIds.length === 0) continue;

    const target = resolvePinViewTargetFromCache({
      graphId,
      pinId: pin.pinId,
      direction: 'input',
      isExec: pin.isExec,
      connectionIds: pin.connectionIds,
      pinResults,
    });
    if (!target) continue;

    entries.push(
      buildPinResultSearchEntry(
        `input:${pin.pinId}`,
        'input',
        target.pinResult,
        resolveLabels(pin.nodeId, pin.pinId),
      ),
    );
  }

  return entries.sort((left, right) => {
    if (left.direction !== right.direction) {
      return left.direction === 'output' ? -1 : 1;
    }
    return left.searchText.localeCompare(right.searchText);
  });
}

export function filterPinResultSearchEntries(
  entries: PinResultSearchEntry[],
  query: string,
): PinResultSearchEntry[] {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return entries;
  return entries.filter((entry) => entry.searchText.includes(normalized));
}
