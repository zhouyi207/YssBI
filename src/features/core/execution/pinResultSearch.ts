import {
  outputPinRef,
  type InspectableResultRef,
} from "@/features/domain/result/inspectableResultRef";
import type { PinResultProjection } from "./executionTypes";
import { pinResultCacheKey } from "./pinResultIndex";

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
  projection: PinResultProjection,
  labels: PinResultSearchLabels,
): PinResultSearchEntry | null {
  const selected = projection.result;
  if (!selected) return null;

  const nodeTitle = labels.nodeTitle.trim();
  const pinName = labels.pinName.trim();
  const sourceTitle = `${selected.state.kind} · ${selected.resultId}`;
  const searchText = [
    nodeTitle,
    pinName,
    sourceTitle,
    projection.graphPath,
    selected.provenance.runId,
  ]
    .join(" ")
    .toLowerCase();

  return {
    id: pinResultCacheKey(projection.graphPath, projection.output),
    ref: outputPinRef(projection.graphPath, projection.output),
    nodeTitle,
    pinName,
    sourceTitle,
    searchText,
  };
}

export function collectPinResultSearchEntries(
  results: ReadonlyMap<string, PinResultProjection>,
  resolveLabels: (projection: PinResultProjection) => PinResultSearchLabels,
): PinResultSearchEntry[] {
  return [...results.values()]
    .flatMap((projection) => {
      const entry = buildPinResultSearchEntry(projection, resolveLabels(projection));
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
