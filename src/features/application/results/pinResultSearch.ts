import {
  outputPinRef,
  type InspectableResultRef,
} from "@/features/domain/result/inspectableResultRef";
import { graphOutputKey } from "@/features/domain/editorProjection";
import type { ResultDescriptor } from "./types";
import type { DeepReadonly } from "@/shared/types/deepReadonly";

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
  result: DeepReadonly<ResultDescriptor>,
  labels: PinResultSearchLabels,
): PinResultSearchEntry | null {
  const output = result.provenance.output;
  if (!output) return null;

  const nodeTitle = labels.nodeTitle.trim();
  const pinName = labels.pinName.trim();
  const sourceTitle = result.title;
  const searchText = [nodeTitle, pinName, sourceTitle, output.graphPath, result.provenance.runId]
    .join(" ")
    .toLowerCase();

  return {
    id: graphOutputKey(output),
    ref: outputPinRef(output.graphPath, output.port),
    nodeTitle,
    pinName,
    sourceTitle,
    searchText,
  };
}

export function collectPinResultSearchEntries(
  results: readonly DeepReadonly<ResultDescriptor>[],
  resolveLabels: (result: DeepReadonly<ResultDescriptor>) => PinResultSearchLabels,
): PinResultSearchEntry[] {
  return results
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
