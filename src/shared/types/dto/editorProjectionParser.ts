import { isEditorGraphProjectionDto } from './editorProjectionGuards';
import { isSchemaAwareParameterEditorDto } from './parameterEditorValidators';
import type {
  EditorGraphProjectionDto,
  PortAddressDto,
} from './editorProjection';

export function parseEditorGraphProjectionDto(value: unknown): EditorGraphProjectionDto {
  if (!isEditorGraphProjectionDto(value)) {
    throw new Error('Invalid editor graph projection response');
  }
  return validateEditorGraphProjection(value);
}

export function validateEditorGraphProjection(
  projection: EditorGraphProjectionDto,
): EditorGraphProjectionDto {
  if (projection.basis.graphPath !== projection.graphPath) {
    throw new Error(
      `projection basis graph path '${projection.basis.graphPath}' does not match projection graph path '${projection.graphPath}'`,
    );
  }
  if (projection.basis.graphRevision !== projection.sourceRevision) {
    throw new Error(
      `projection basis revision ${projection.basis.graphRevision} does not match source revision ${projection.sourceRevision}`,
    );
  }

  const nodeIds = new Set<string>();
  const portDirections = new Map<string, 'input' | 'output'>();
  for (const node of projection.nodes) {
    validateNode(projection, nodeIds, portDirections, node);
  }

  const connectionIds = new Set<string>();
  for (const connection of projection.connections) {
    if (connectionIds.has(connection.connectionId)) {
      throw new Error(`projection contains duplicate connection '${connection.connectionId}'`);
    }
    connectionIds.add(connection.connectionId);

    const outputDirection = portDirections.get(portAddressKey(connection.output));
    const inputDirection = portDirections.get(portAddressKey(connection.input));
    if (!outputDirection || !inputDirection) {
      throw new Error(
        `projection connection '${connection.connectionId}' references a missing port`,
      );
    }
    if (outputDirection !== 'output' || inputDirection !== 'input') {
      throw new Error(
        `projection connection '${connection.connectionId}' endpoint direction is invalid`,
      );
    }
  }

  return projection;
}

function validateNode(
  projection: EditorGraphProjectionDto,
  nodeIds: Set<string>,
  portDirections: Map<string, 'input' | 'output'>,
  node: EditorGraphProjectionDto['nodes'][number],
): void {
  if (nodeIds.has(node.nodeId)) {
    throw new Error(`projection contains duplicate node '${node.nodeId}'`);
  }
  nodeIds.add(node.nodeId);

  if (node.graphPath !== projection.graphPath) {
    throw new Error(
      `projection node '${node.nodeId}' graph path '${node.graphPath}' does not match projection graph path '${projection.graphPath}'`,
    );
  }
  if (node.sourceRevision !== projection.sourceRevision) {
    throw new Error(
      `projection node '${node.nodeId}' revision ${node.sourceRevision} does not match source revision ${projection.sourceRevision}`,
    );
  }

  for (const parameter of node.parameterEditors) {
    if (parameter.configuration !== null
      && !isSchemaAwareParameterEditorDto(parameter.configuration)) {
      throw new Error(`projection parameter editor '${parameter.key}' is invalid`);
    }
  }

  for (const port of node.ports) {
    if (port.address.nodeId !== node.nodeId) {
      throw new Error(
        `projection port is owned by node '${port.address.nodeId}' but is contained by node '${node.nodeId}'`,
      );
    }

    const key = portAddressKey(port.address);
    if (portDirections.has(key)) {
      throw new Error(`projection contains duplicate port '${key}'`);
    }
    portDirections.set(key, port.direction);
  }
}

function portAddressKey(address: PortAddressDto): string {
  return address.kind === 'declared'
    ? JSON.stringify(['declared', address.nodeId, address.portKey])
    : JSON.stringify(['instance', address.nodeId, address.templateKey, address.instanceId]);
}
