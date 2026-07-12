import { useMemo } from 'react';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import {
  collectPinResultSearchEntries,
  filterPinResultSearchEntries,
  type PinResultSearchEntry,
} from '@/features/core/execution/pinResultSearch';
import { useExecutionStore } from '@/features/core/execution';

export function usePinResultSearch(graphPath: string, query: string) {
  const pinResultCount = useExecutionStore(
    (state) => state.graphs[graphPath]?.pinResults?.size ?? 0,
  );
  const graphBucket = useGraphDataStore((state) => state.graphEntities[graphPath]);

  const entries = useMemo(() => {
    const pinResults = useExecutionStore.getState().graphs[graphPath]?.pinResults;
    if (!pinResults || pinResults.size === 0) return [];

    const resolveLabels = (labelGraphPath: string, nodeId: string, pinId: string) => {
      const graphStore = useGraphDataStore.getState();
      const node = graphStore.getGraphNode(labelGraphPath, nodeId);
      const pin = graphStore.getGraphPin(labelGraphPath, pinId);
      return {
        nodeTitle: node?.title ?? nodeId,
        pinName: pin?.name ?? pinId,
      };
    };

    return collectPinResultSearchEntries(pinResults, resolveLabels);
  }, [graphPath, graphBucket, pinResultCount]);

  const filteredEntries = useMemo(
    () => filterPinResultSearchEntries(entries, query),
    [entries, query],
  );

  return {
    hasResults: pinResultCount > 0,
    entries: filteredEntries,
  };
}

export type { PinResultSearchEntry };
