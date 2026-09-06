import { useMemo } from "react";
import {
  nodeDisplayTitle,
  pinDisplayTitle,
  portAddressKey,
} from "@/features/domain/editorProjection";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import {
  collectPinResultSearchEntries,
  filterPinResultSearchEntries,
  type PinResultSearchEntry,
} from "@/features/core/execution/pinResultSearch";
import { useExecutionStore } from "@/features/core/execution";

export function usePinResultSearch(graphPath: string, query: string) {
  const results = useExecutionStore((state) => state.graphs[graphPath]?.pinResults);
  const graphBucket = useGraphProjectionStore((state) => state.graphEntities[graphPath]);

  const entries = useMemo(() => {
    if (!results || results.size === 0) return [];

    return collectPinResultSearchEntries(results, (projection) => {
      const graphStore = useGraphProjectionStore.getState();
      const node = graphStore.getGraphNode(projection.graphPath, projection.output.nodeId);
      const pin = graphStore.getGraphPin(projection.graphPath, portAddressKey(projection.output));
      return {
        nodeTitle: nodeDisplayTitle(node) ?? "",
        pinName: pinDisplayTitle(pin) ?? "",
      };
    });
  }, [graphBucket, results]);

  const filteredEntries = useMemo(
    () => filterPinResultSearchEntries(entries, query),
    [entries, query],
  );

  return {
    hasResults: (results?.size ?? 0) > 0,
    entries: filteredEntries,
  };
}

export type { PinResultSearchEntry };
