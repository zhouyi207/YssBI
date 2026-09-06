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
} from "@/features/application/results/pinResultSearch";
import { useResultDescriptors } from "@/features/application/results/runtime";

export function usePinResultSearch(graphPath: string, query: string) {
  const descriptors = useResultDescriptors();
  const graphBucket = useGraphProjectionStore((state) => state.graphEntities[graphPath]);

  const entries = useMemo(() => {
    const results = Object.values(descriptors).filter(
      (result) => result?.provenance.output?.graphPath === graphPath,
    );

    return collectPinResultSearchEntries(
      results.filter((result) => result !== null),
      (result) => {
        const graphStore = useGraphProjectionStore.getState();
        const node = graphStore.getGraphNode(result.provenance.graphPath, result.provenance.nodeId);
        const pin = graphStore.getGraphPin(
          result.provenance.graphPath,
          result.provenance.output ? portAddressKey(result.provenance.output.port) : "",
        );
        return {
          nodeTitle: nodeDisplayTitle(node) ?? "",
          pinName: pinDisplayTitle(pin) ?? "",
        };
      },
    );
  }, [graphBucket, descriptors, graphPath]);

  const filteredEntries = useMemo(
    () => filterPinResultSearchEntries(entries, query),
    [entries, query],
  );

  return {
    hasResults: entries.length > 0,
    entries: filteredEntries,
  };
}

export type { PinResultSearchEntry };
