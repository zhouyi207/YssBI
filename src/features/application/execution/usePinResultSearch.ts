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
import { isExecPin } from '@/shared/types/domain/pinSemantics';

const EMPTY_PIN_RESULTS = new Map<string, PinResultState>();

function collectGraphPins(graphPath: string): PinResultSearchPinRef[] {
  const graphStore = useGraphDataStore.getState();
  const nodeIds = graphStore.getGraphNodeIds(graphPath);
  const pins: PinResultSearchPinRef[] = [];

  for (const nodeId of nodeIds) {
    for (const pinId of graphStore.getGraphNodePins(graphPath, nodeId)) {
      const pin = graphStore.getGraphPin(graphPath, pinId);
      if (!pin) continue;

      pins.push({
        pinId: pin.id,
        nodeId: pin.nodeId,
        direction: pin.direction,
        isExec: isExecPin(pin),
        connectionIds: graphStore.getGraphPinConnections(graphPath, pinId),
      });
    }
  }

  return pins;
}

export function usePinResultSearch(graphPath: string, query: string) {
  const pinResults = useExecutionStore(
    (state) => state.graphs[graphPath]?.pinResults ?? EMPTY_PIN_RESULTS,
  );
  const graphBucket = useGraphDataStore((state) => state.graphEntities[graphPath]);

  const entries = useMemo(() => {
    if (pinResults.size === 0 || !graphBucket) return [];

    const resolveLabels = (nodeId: string, pinId: string) => {
      const graphStore = useGraphDataStore.getState();
      const node = graphStore.getGraphNode(graphPath, nodeId);
      const pin = graphStore.getGraphPin(graphPath, pinId);
      return {
        nodeTitle: node?.title ?? nodeId,
        pinName: pin?.name ?? pinId,
      };
    };

    return collectPinResultSearchEntries(
      graphPath,
      pinResults,
      collectGraphPins(graphPath),
      resolveLabels,
    );
  }, [graphPath, graphBucket, pinResults]);

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
