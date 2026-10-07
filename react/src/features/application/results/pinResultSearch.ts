import {
  outputPinRef,
  type InspectableResultRef,
} from "@/features/domain/result/inspectableResultRef";
import {
  graphOutputKey,
  nodeDisplayTitle,
  pinDisplayTitle,
  portAddressKey,
} from "@/features/domain/editorProjection";
import type { NodePinDisplayBucket } from "@/features/domain/editorProjection/displayLabels";
import { shallow } from "zustand/shallow";
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

export function createPinResultSearchProjector() {
  let previousResults: readonly DeepReadonly<ResultDescriptor>[] | undefined;
  let previousNodes: NodePinDisplayBucket["nodes"];
  let previousPins: NodePinDisplayBucket["pins"];
  let entries: PinResultSearchEntry[] = [];
  const cache = new Map<string, { runId: string; entry: PinResultSearchEntry }>();
  return (
    results: readonly DeepReadonly<ResultDescriptor>[],
    graph: NodePinDisplayBucket | undefined,
  ) => {
    if (
      results === previousResults &&
      graph?.nodes === previousNodes &&
      graph?.pins === previousPins
    )
      return entries;
    const next: PinResultSearchEntry[] = [];
    const visible = new Set<string>();
    for (const result of results) {
      const output = result.provenance.output;
      if (!output) continue;
      const id = graphOutputKey(output);
      visible.add(id);
      const nodeTitle = nodeDisplayTitle(graph?.nodes?.[result.provenance.nodeId]) ?? "";
      const pinName = pinDisplayTitle(graph?.pins?.[portAddressKey(output.port)]) ?? "";
      const previous = cache.get(id);
      let entry = previous?.entry;
      if (
        !entry ||
        previous?.runId !== result.provenance.runId ||
        entry.nodeTitle !== nodeTitle ||
        entry.pinName !== pinName ||
        entry.sourceTitle !== result.title
      ) {
        entry = buildPinResultSearchEntry(result, { nodeTitle, pinName })!;
        cache.set(id, { runId: result.provenance.runId, entry });
      }
      next.push(entry);
    }
    for (const id of cache.keys()) if (!visible.has(id)) cache.delete(id);
    next.sort((left, right) => left.searchText.localeCompare(right.searchText));
    if (!shallow(entries, next)) entries = next;
    previousResults = results;
    previousNodes = graph?.nodes;
    previousPins = graph?.pins;
    return entries;
  };
}

export function filterPinResultSearchEntries(
  entries: readonly PinResultSearchEntry[],
  query: string,
): readonly PinResultSearchEntry[] {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return entries;
  return entries.filter((entry) => entry.searchText.includes(normalized));
}
