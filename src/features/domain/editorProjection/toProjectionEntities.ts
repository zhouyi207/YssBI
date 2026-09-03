import type { EditorGraphProjectionDto } from "@/shared/types/domain/editorProjection";
import { validateEditorGraphProjection } from "@/shared/types/domain/editorProjectionParser";
import { portAddressKey } from "./portAddressKey";
import type { EditorProjectionEntities } from "./types";

function emptyRecord<T>(): Record<string, T> {
  return Object.create(null) as Record<string, T>;
}

export function toProjectionEntities(
  projection: EditorGraphProjectionDto,
): EditorProjectionEntities {
  validateEditorGraphProjection(projection);

  const nodes = emptyRecord<EditorProjectionEntities["nodes"][string]>();
  const ports = emptyRecord<EditorProjectionEntities["ports"][string]>();
  const connections = emptyRecord<EditorProjectionEntities["connections"][string]>();
  const portIdsByNodeId = emptyRecord<string[]>();
  const connectionIdsByPortId = emptyRecord<string[]>();

  for (const node of projection.nodes) {
    const { ports: nodePorts, ...nodeEntity } = node;
    nodes[node.nodeId] = nodeEntity;
    portIdsByNodeId[node.nodeId] = [];

    for (const port of nodePorts) {
      const portId = portAddressKey(port.address);
      ports[portId] = port;
      portIdsByNodeId[node.nodeId].push(portId);
      connectionIdsByPortId[portId] = [];
    }
  }

  for (const connection of projection.connections) {
    connections[connection.connectionId] = connection;
    connectionIdsByPortId[portAddressKey(connection.output)].push(connection.connectionId);
    connectionIdsByPortId[portAddressKey(connection.input)].push(connection.connectionId);
  }

  return {
    basis: projection.basis,
    graphPath: projection.graphPath,
    sourceRevision: projection.sourceRevision,
    nodes,
    ports,
    connections,
    portIdsByNodeId,
    connectionIdsByPortId,
    diagnostics: projection.diagnostics,
    outcome: projection.outcome,
    hasBlockingDiagnostics: projection.hasBlockingDiagnostics,
  };
}
