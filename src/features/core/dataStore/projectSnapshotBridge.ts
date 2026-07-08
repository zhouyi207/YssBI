/**
 * 从多个 store 组装图导出快照（exportSnapshot 专用）。
 * 跨 store 读取集中于此，避免 projectIOStore 隐式依赖未 import 的 hook。
 */
import { getViewport } from '@/features/core/viewport';
import { resourceKey, useResourceStore } from '@/features/core/resource';
import type { GraphData } from '@/shared/types/store/graph';
import { useGraphDataStore } from './graphDataStore';
import { buildGraphSnapshot } from './projectSnapshot';

export function buildGraphSnapshotFromStores(): Record<string, GraphData> {
  const resourceStore = useResourceStore.getState();
  const dataStore = useGraphDataStore.getState();

  return buildGraphSnapshot({
    graphOrder: resourceStore.graphOrder,
    getResourceMeta: (graphId) => {
      const eventMeta = resourceStore.resources[resourceKey({ id: graphId, kind: 'event' })];
      const functionMeta = resourceStore.resources[resourceKey({ id: graphId, kind: 'function' })];
      const meta = eventMeta ?? functionMeta;
      return meta ? { name: meta.name, kind: meta.kind, exists: meta.exists } : null;
    },
    getGraphNodeIds: (graphId) => dataStore.getGraphNodeIds(graphId),
    getGraphNode: (graphId, nodeId) => dataStore.getGraphNode(graphId, nodeId) ?? null,
    getGraphNodePins: (graphId, nodeId) => dataStore.getGraphNodePins(graphId, nodeId),
    getGraphPin: (graphId, pinId) => dataStore.getGraphPin(graphId, pinId) ?? null,
    getGraphPinConnections: (graphId, pinId) => dataStore.getGraphPinConnections(graphId, pinId),
    getGraphConnection: (graphId, connectionId) =>
      dataStore.getGraphConnection(graphId, connectionId) ?? null,
    getViewport,
  });
}
