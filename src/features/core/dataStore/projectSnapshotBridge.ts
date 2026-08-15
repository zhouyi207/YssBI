/**
 * 从多个 store 组装图导出快照（exportSnapshot 专用）。
 * 跨 store 读取集中于此，避免 projectIOStore 隐式依赖未 import 的 hook。
 */
import { resourceKey, useResourceStore } from '@/features/core/resource';
import type { GraphSnapshotData } from '@/shared/types/store/graph';
import { useGraphDataStore } from './graphDataStore';
import { useGraphMetaStore } from './graphMetaStore';
import { buildGraphSnapshot } from './projectSnapshot';

export function buildGraphSnapshotFromStores(): Record<string, GraphSnapshotData> {
  const resourceStore = useResourceStore.getState();
  const dataStore = useGraphDataStore.getState();
  const metaStore = useGraphMetaStore.getState();

  return buildGraphSnapshot({
    graphOrder: resourceStore.graphOrder,
    getResourceMeta: (graphPath) => {
      const eventMeta = resourceStore.resources[resourceKey({ id: graphPath, kind: 'event' })];
      const functionMeta = resourceStore.resources[resourceKey({ id: graphPath, kind: 'function' })];
      const meta = eventMeta ?? functionMeta;
      return meta ? { name: meta.name, kind: meta.kind, exists: meta.exists } : null;
    },
    getFunctionSignature: (graphPath) => {
      const meta = metaStore.graphs[graphPath];
      if (!meta || meta.type !== 'function') return null;
      return {
        functionInputs: meta.functionInputs ?? [],
        functionOutputs: meta.functionOutputs ?? [],
      };
    },
    getGraphNodeIds: (graphPath) => dataStore.getGraphNodeIds(graphPath),
    getGraphNode: (graphPath, nodeId) => dataStore.getGraphNode(graphPath, nodeId) ?? null,
    getGraphNodePins: (graphPath, nodeId) => dataStore.getGraphNodePins(graphPath, nodeId),
    getGraphPin: (graphPath, pinId) => dataStore.getGraphPin(graphPath, pinId) ?? null,
    getGraphPinConnections: (graphPath, pinId) => dataStore.getGraphPinConnections(graphPath, pinId),
    getGraphConnection: (graphPath, connectionId) =>
      dataStore.getGraphConnection(graphPath, connectionId) ?? null,
  });
}
