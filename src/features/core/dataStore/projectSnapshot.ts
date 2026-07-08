import type { GraphPosition } from '@/shared/types/domain/graph';
import type { GraphData } from '@/shared/types/store/graph';
import type { ProjectResourceMeta } from '@/features/core/resource/resourceTypes';

/** 图快照读取端口（纯函数测试与 projectIOStore 共用） */
export interface GraphSnapshotAccess {
  graphOrder: string[];
  getResourceMeta(graphId: string): Pick<ProjectResourceMeta, 'name' | 'kind' | 'exists'> | null;
  getGraphNodeIds(graphId: string): string[];
  getGraphNode(graphId: string, nodeId: string): GraphData['nodes'][number] | null;
  getGraphNodePins(graphId: string, nodeId: string): string[];
  getGraphPin(graphId: string, pinId: string): GraphData['pins'][number] | null;
  getGraphPinConnections(graphId: string, pinId: string): string[];
  getGraphConnection(
    graphId: string,
    connectionId: string,
  ): { from: string; to: string } | null;
  getViewport(graphId: string): GraphPosition;
}

/** 从 store 状态导出图快照（store 内 `ConnectionData[]`；持久化经 `graphDataToDomainGraph` 包装） */
export function buildGraphSnapshot(access: GraphSnapshotAccess): Record<string, GraphData> {
  return Object.fromEntries(
    access.graphOrder
      .map((graphId) => {
        const meta = access.getResourceMeta(graphId);
        if (!meta?.exists) return null;

        const nodeIds = access.getGraphNodeIds(graphId);
        const nodes = nodeIds
          .map((nodeId) => access.getGraphNode(graphId, nodeId))
          .filter((node): node is NonNullable<typeof node> => node != null);
        const pins = nodeIds.flatMap((nodeId) =>
          access
            .getGraphNodePins(graphId, nodeId)
            .map((pinId) => access.getGraphPin(graphId, pinId))
            .filter((pin): pin is NonNullable<typeof pin> => pin != null),
        );
        const connectionIds = new Set<string>();
        for (const pin of pins) {
          for (const connectionId of access.getGraphPinConnections(graphId, pin.id)) {
            connectionIds.add(connectionId);
          }
        }
        const connections = Array.from(connectionIds)
          .map((connectionId) => access.getGraphConnection(graphId, connectionId))
          .filter((connection): connection is NonNullable<typeof connection> => connection != null)
          .map((connection) => ({
            id: `${connection.from}->${connection.to}`,
            from: connection.from,
            to: connection.to,
          }));

        const graph: GraphData = {
          id: graphId,
          name: meta.name,
          type: meta.kind === 'function' ? 'function' : 'event',
          nodes,
          pins,
          connections,
          canvas: access.getViewport(graphId),
        };
        return [graphId, graph] as const;
      })
      .filter((entry): entry is [string, GraphData] => entry !== null),
  );
}
