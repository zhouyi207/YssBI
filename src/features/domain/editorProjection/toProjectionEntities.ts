import type { EditorGraphProjectionDto } from '@/shared/types/dto/editorProjection';
import { validateEditorGraphProjection } from '@/shared/types/dto/editorProjectionParser';
import { portAddressKey } from './portAddressKey';
import type { EditorProjectionEntities } from './types';

function emptyRecord<T>(): Record<string, T> {
  return Object.create(null) as Record<string, T>;
}

export function toProjectionEntities(
  projection: EditorGraphProjectionDto,
): EditorProjectionEntities {
  validateEditorGraphProjection(projection);

  const nodes = emptyRecord<EditorProjectionEntities['nodes'][string]>();
  const ports = emptyRecord<EditorProjectionEntities['ports'][string]>();
  const connections = emptyRecord<EditorProjectionEntities['connections'][string]>();
  const portKeysByNodeId = emptyRecord<string[]>();
  const connectionIdsByPortKey = emptyRecord<string[]>();

  for (const node of projection.nodes) {
    const { ports: nodePorts, ...nodeEntity } = node;
    nodes[node.nodeId] = nodeEntity;
    portKeysByNodeId[node.nodeId] = [];

    for (const port of nodePorts) {
      const key = portAddressKey(port.address);
      ports[key] = port;
      portKeysByNodeId[node.nodeId].push(key);
      connectionIdsByPortKey[key] = [];
    }
  }

  for (const connection of projection.connections) {
    connections[connection.connectionId] = connection;
    connectionIdsByPortKey[portAddressKey(connection.output)].push(connection.connectionId);
    connectionIdsByPortKey[portAddressKey(connection.input)].push(connection.connectionId);
  }

  return {
    basis: projection.basis,
    graphPath: projection.graphPath,
    sourceRevision: projection.sourceRevision,
    nodes,
    ports,
    connections,
    portKeysByNodeId,
    connectionIdsByPortKey,
    diagnostics: projection.diagnostics,
    hasBlockingDiagnostics: projection.hasBlockingDiagnostics,
  };
}
