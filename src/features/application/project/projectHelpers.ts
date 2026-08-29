/** Application-owned project queries used by editor and execution workflows. */

import { resourceKey, useResourceStore } from '@/features/core/resource';
import type { GraphSnapshotData } from '@/shared/types/store/graph';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useGraphMetaStore } from '@/features/core/dataStore/graphMetaStore';
import { buildGraphSnapshot } from '@/features/core/dataStore/projectSnapshot';

/** Assemble a graph projection from the frontend stores owned by Core. */
export function getGraphByPath(graphPath: string): GraphSnapshotData | null {
  const resourceStore = useResourceStore.getState();
  const dataStore = useGraphDataStore.getState();
  const metaStore = useGraphMetaStore.getState();
  const snapshots = buildGraphSnapshot({
    graphOrder: resourceStore.graphOrder,
    getResourceMeta: (path) => {
      const eventMeta = resourceStore.resources[resourceKey({ id: path, kind: 'event' })];
      const functionMeta = resourceStore.resources[resourceKey({ id: path, kind: 'function' })];
      const meta = eventMeta ?? functionMeta;
      return meta ? { name: meta.name, kind: meta.kind, exists: meta.exists } : null;
    },
    getFunctionSignature: (path) => {
      const meta = metaStore.graphs[path];
      if (!meta || meta.type !== 'function') return null;
      return {
        functionInputs: meta.functionInputs ?? [],
        functionOutputs: meta.functionOutputs ?? [],
      };
    },
    getGraphNodeIds: (path) => dataStore.getGraphNodeIds(path),
    getGraphNode: (path, nodeId) => dataStore.getGraphNode(path, nodeId) ?? null,
    getGraphNodePins: (path, nodeId) => dataStore.getGraphNodePins(path, nodeId),
    getGraphPin: (path, pinId) => dataStore.getGraphPin(path, pinId) ?? null,
    getGraphPinConnections: (path, pinId) => dataStore.getGraphPinConnections(path, pinId),
    getGraphConnection: (path, connectionId) =>
      dataStore.getGraphConnection(path, connectionId) ?? null,
  });
  return snapshots[graphPath] ?? null;
}
