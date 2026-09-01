import { useMemo } from "react";
import {
  nodeDisplayTitle,
  pinDisplayTitle,
  portAddressKey,
} from "@/features/domain/editorProjection";
import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import {
  collectPinResultSearchEntries,
  filterPinResultSearchEntries,
  type PinResultSearchEntry,
} from "@/features/core/execution/pinResultSearch";
import { useExecutionStore } from "@/features/core/execution";

export function usePinResultSearch(graphPath: string, query: string) {
  const historyCount = useExecutionStore(
    (state) => state.graphs[graphPath]?.pinHistories.size ?? 0,
  );
  const graphBucket = useGraphDataStore((state) => state.graphEntities[graphPath]);

  const entries = useMemo(() => {
    const histories = useExecutionStore.getState().graphs[graphPath]?.pinHistories;
    if (!histories || histories.size === 0) return [];

    return collectPinResultSearchEntries(histories, (history) => {
      const graphStore = useGraphDataStore.getState();
      const node = graphStore.getGraphNode(history.graphPath, history.output.nodeId);
      const pin = graphStore.getGraphPin(history.graphPath, portAddressKey(history.output));
      return {
        nodeTitle: nodeDisplayTitle(node) ?? "",
        pinName: pinDisplayTitle(pin) ?? "",
      };
    });
  }, [graphPath, graphBucket, historyCount]);

  const filteredEntries = useMemo(
    () => filterPinResultSearchEntries(entries, query),
    [entries, query],
  );

  return {
    hasResults: historyCount > 0,
    entries: filteredEntries,
  };
}

export type { PinResultSearchEntry };
