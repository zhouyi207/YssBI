import type { FunctionSignaturePin } from "@/shared/types/domain/graph";
import type { GraphData, GraphSnapshotData } from "@/shared/types/store/graph";
import type { ProjectResourceMeta } from "@/features/core/resource/resourceTypes";

export type FunctionSignatureSnapshot = {
  functionInputs: FunctionSignaturePin[];
  functionOutputs: FunctionSignaturePin[];
};

/** 图快照读取端口（纯函数测试与 projectIOStore 共用） */
export interface GraphSnapshotAccess {
  graphOrder: string[];
  getResourceMeta(graphPath: string): Pick<ProjectResourceMeta, "name" | "kind" | "exists"> | null;
  getFunctionSignature?(graphPath: string): FunctionSignatureSnapshot | null;
  getGraphNodeIds(graphPath: string): string[];
  getGraphNode(graphPath: string, nodeId: string): GraphData["nodes"][number] | null;
  getGraphNodePins(graphPath: string, nodeId: string): string[];
  getGraphPin(graphPath: string, pinId: string): GraphData["pins"][number] | null;
  getGraphPinConnections(graphPath: string, pinId: string): string[];
  getGraphConnection(graphPath: string, connectionId: string): { from: string; to: string } | null;
}

/** 从 store 状态导出图快照；连接只保留 domain 序列化所需的端点。 */
export function buildGraphSnapshot(access: GraphSnapshotAccess): Record<string, GraphSnapshotData> {
  return Object.fromEntries(
    access.graphOrder
      .map((graphPath) => {
        const meta = access.getResourceMeta(graphPath);
        if (!meta?.exists) return null;

        const nodeIds = access.getGraphNodeIds(graphPath);
        const nodes = nodeIds
          .map((nodeId) => access.getGraphNode(graphPath, nodeId))
          .filter((node): node is NonNullable<typeof node> => node != null);
        const pins = nodeIds.flatMap((nodeId) =>
          access
            .getGraphNodePins(graphPath, nodeId)
            .map((pinId) => access.getGraphPin(graphPath, pinId))
            .filter((pin): pin is NonNullable<typeof pin> => pin != null),
        );
        const connectionIds = new Set<string>();
        for (const pin of pins) {
          for (const connectionId of access.getGraphPinConnections(graphPath, pin.id)) {
            connectionIds.add(connectionId);
          }
        }
        const connections = Array.from(connectionIds)
          .map((connectionId) => access.getGraphConnection(graphPath, connectionId))
          .filter((connection): connection is NonNullable<typeof connection> => connection != null);

        const graph: GraphSnapshotData = {
          path: graphPath,
          name: meta.name,
          type: meta.kind === "function" ? "function" : "event",
          nodes,
          pins,
          connections,
        };

        if (graph.type === "function" && access.getFunctionSignature) {
          const signature = access.getFunctionSignature(graphPath);
          if (signature) {
            graph.functionInputs = signature.functionInputs;
            graph.functionOutputs = signature.functionOutputs;
          }
        }

        return [graphPath, graph] as const;
      })
      .filter((entry): entry is [string, GraphData] => entry !== null),
  );
}
