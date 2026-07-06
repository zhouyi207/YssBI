import { useMemo } from 'react';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import {
  collectPinResultSearchEntries,
  filterPinResultSearchEntries,
  type PinResultSearchEntry,
  type PinResultSearchPinRef,
} from '@/features/core/execution/pinResultSearch';
import { useExecutionStore } from '@/features/core/execution';
import type { PinResultState } from '@/shared/types/ui';

const EMPTY_PIN_RESULTS = new Map<string, PinResultState>();

function collectGraphPins(graphId: string): PinResultSearchPinRef[] {
  const graphStore = useGraphDataStore.getState();
  const nodeIds = graphStore.getGraphNodeIds(graphId);
  const pins: PinResultSearchPinRef[] = [];

  for (const nodeId of nodeIds) {
    for (const pinId of graphStore.getGraphNodePins(graphId, nodeId)) {
      const pin = graphStore.getGraphPin(graphId, pinId);
      if (!pin) continue;

      pins.push({
        pinId: pin.id,
        nodeId: pin.nodeId,
        direction: pin.direction,
        pinType: pin.type,
        connectionIds: graphStore.getGraphPinConnections(graphId, pinId),
      });
    }
  }

  return pins;
}

export function usePinResultSearch(graphId: string, query: string) {
  const pinResults = useExecutionStore(
    (state) => state.graphs[graphId]?.pinResults ?? EMPTY_PIN_RESULTS,
  );
  const graphBucket = useGraphDataStore((state) => state.graphEntities[graphId]);

  const entries = useMemo(() => {
    if (pinResults.size === 0 || !graphBucket) return [];

    const resolveLabels = (nodeId: string, pinId: string) => {
      const graphStore = useGraphDataStore.getState();
      const node = graphStore.getGraphNode(graphId, nodeId);
      const pin = graphStore.getGraphPin(graphId, pinId);
      return {
        nodeTitle: node?.title ?? nodeId,
        pinName: pin?.name ?? pinId,
      };
    };

    return collectPinResultSearchEntries(
      graphId,
      pinResults,
      collectGraphPins(graphId),
      resolveLabels,
    );
  }, [graphId, graphBucket, pinResults]);

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
