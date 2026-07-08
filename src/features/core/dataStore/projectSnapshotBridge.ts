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
    getResourceMeta: (graphPath) => {
      const eventMeta = resourceStore.resources[resourceKey({ id: graphPath, kind: 'event' })];
      const functionMeta = resourceStore.resources[resourceKey({ id: graphPath, kind: 'function' })];
      const meta = eventMeta ?? functionMeta;
      return meta ? { name: meta.name, kind: meta.kind, exists: meta.exists } : null;
    },
    getGraphNodeIds: (graphPath) => dataStore.getGraphNodeIds(graphPath),
    getGraphNode: (graphPath, nodeId) => dataStore.getGraphNode(graphPath, nodeId) ?? null,
    getGraphNodePins: (graphPath, nodeId) => dataStore.getGraphNodePins(graphPath, nodeId),
    getGraphPin: (graphPath, pinId) => dataStore.getGraphPin(graphPath, pinId) ?? null,
    getGraphPinConnections: (graphPath, pinId) => dataStore.getGraphPinConnections(graphPath, pinId),
    getGraphConnection: (graphPath, connectionId) =>
      dataStore.getGraphConnection(graphPath, connectionId) ?? null,
    getViewport,
  });
}
